use std::collections::HashMap;

use clap::Parser;
use nacos_sdk::api::config::ConfigService;
use serde_yaml::Value;

use nacos_sync::config::{Args, Config};
use nacos_sync::nacos::{build_config_service, get_all_data_id};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config_path = args.config_path.unwrap_or_else(|| "sync-sync_config.yaml".to_string());
    let sync_config = Config::read_config(config_path.as_str())?;

    let from = &sync_config.from;
    let from_config_service = build_config_service(from)?;

    let to = &sync_config.to;
    let to_config_service = build_config_service(to)?;

    let all_data_id = get_all_data_id(from).await?;
    println!("All Data ID For Yaml Config: {:#?}", all_data_id);

    do_sync(all_data_id, &sync_config, Box::new(from_config_service), Box::new(to_config_service)).await?;

    Ok(())
}


async fn do_sync(
    all_data_id: Vec<String>,
    sync_config: &Config,
    from_config_service: Box<dyn ConfigService>,
    to_config_service: Box<dyn ConfigService>)
    -> Result<(), Box<dyn std::error::Error>> {
    let ignore_vec = if let Some(vec) = sync_config.ignore.clone() {
        vec
    } else { vec![] };
    let default_group = "DEFAULT_GROUP".to_string();
    let default_config_type = "yaml".to_string();
    for data_id in all_data_id {
        let config_resp = from_config_service.get_config(data_id.clone(), default_group.clone()).await?;
        let contents = config_resp.content();
        let yaml_config: HashMap<String, Value> = serde_yaml::from_str(contents)?;

        let mut result = yaml_config;
        if ignore_vec.iter().any(|ignore| &ignore.data_id == &data_id) {
            let map = ignore_vec.first().unwrap();
            let ignore: HashMap<String, Value> = map.fields.clone();
            println!("Ignore: {:#?} for Data ID: {:#?}", &ignore, &data_id);
            result = filter_config(&result, &ignore);
        }

        let new_config_content = serde_yaml::to_string(&result).unwrap();

        let sync_response = to_config_service.publish_config(data_id.clone(), default_group.clone(), new_config_content, Some(default_config_type.clone())).await;
        match sync_response {
            Ok(res) => { println!("Sync Success: {}, res: {:#?}", &data_id, res) }
            Err(err) => { println!("Sync Failed: {}, Error: {:#?}", &data_id, &err) }
        }
    }
    Ok(())
}


fn filter_config(
    config: &HashMap<String, Value>,
    ignore: &HashMap<String, Value>,
) -> HashMap<String, Value> {
    config.iter()
        .filter_map(|(key, config_value)| {
            if let Some(ignore_value) = ignore.get(key) {
                match (config_value, ignore_value) {
                    (Value::Mapping(config_map), Value::Mapping(ignore_map)) => {
                        let filtered_sub_map = filter_config(
                            &config_map.iter()
                                .map(|(k, v)| (k.as_str().unwrap().to_string(), v.clone()))
                                .collect(),
                            &ignore_map.iter()
                                .map(|(k, v)| (k.as_str().unwrap().to_string(), v.clone()))
                                .collect(),
                        );

                        if filtered_sub_map.is_empty() {
                            None
                        } else {
                            Some((key.clone(), Value::Mapping(filtered_sub_map.iter()
                                .map(|(k, v)| (Value::String(k.clone()), v.clone()))
                                .collect())))
                        }
                    }
                    _ => None, // 忽略相同路径的配置项
                }
            } else {
                Some((key.clone(), config_value.clone()))
            }
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_config_simple() {
        let config_content = r"
            name:
              first: John
              last: Doe
            ";
        let ignore_content = r"
            name:
              last:
            ";
        let config: HashMap<String, Value> = serde_yaml::from_str(config_content).unwrap();
        let ignore: HashMap<String, Value> = serde_yaml::from_str(ignore_content).unwrap();
        let result = filter_config(&config, &ignore);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("name").unwrap().as_mapping().unwrap().len(), 1);
        assert_eq!(result.get("name").unwrap().as_mapping().unwrap().get("first").unwrap().as_str().unwrap(), "John");
    }

    #[test]
    fn test_filter_config_all_ignored() {
        let config_content = r"
            name:
              last: Doe
            ";
        let ignore_content = r"
            name:
              last:
            ";
        let config: HashMap<String, Value> = serde_yaml::from_str(config_content).unwrap();
        let ignore: HashMap<String, Value> = serde_yaml::from_str(ignore_content).unwrap();
        let result = filter_config(&config, &ignore);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_config_simple_nested() {
        let config_content = r"
           name:
              first: John
              last: Doe
           age:
              max: 30
              min: 18
            ";
        let ignore_content = r"
           name:
              last:
           age:
              max:
            ";
        let config: HashMap<String, Value> = serde_yaml::from_str(config_content).unwrap();
        let ignore: HashMap<String, Value> = serde_yaml::from_str(ignore_content).unwrap();
        let result = filter_config(&config, &ignore);
        println!("Result: {:#?}", &result);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("name").unwrap().as_mapping().unwrap().len(), 1);
        assert_eq!(result.get("name").unwrap().as_mapping().unwrap().get("first").unwrap().as_str().unwrap(), "John");
        assert_eq!(result.get("age").unwrap().as_mapping().unwrap().len(), 1);
        assert_eq!(result.get("age").unwrap().as_mapping().unwrap().get("min").unwrap().as_i64().unwrap(), 18);
    }

    #[test]
    fn test_filter_config_seq() {
        let config_content = r"
           addr:
              - 192.168.1.1
              - 192.168.1.2
              - 192.168.1.3
           port: 8080
            ";
        let ignore_content = r"
           addr:
            ";
        let config: HashMap<String, Value> = serde_yaml::from_str(config_content).unwrap();
        let ignore: HashMap<String, Value> = serde_yaml::from_str(ignore_content).unwrap();
        let result = filter_config(&config, &ignore);
        println!("Result: {:#?}", &result);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("port").unwrap().as_i64().unwrap(), 8080);
    }
}
