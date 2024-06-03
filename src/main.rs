use std::{collections::HashMap, fs::File, hash::Hash, io::Read, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sync_config = read_config("sync-config.yaml").unwrap();

    let config_file_content = read_file("config.yaml").unwrap();
    let config: HashMap<String, Value> = serde_yaml::from_str(&config_file_content)?;

    let ignore_vec = sync_config.ignore;
    let map = ignore_vec.first().unwrap();
    let ignore: HashMap<String, Value> = map.fields.clone();
    println!("Ignore: {:#?}", &ignore);

    let result = filter_config(&config, &ignore);
    println!("Result: {:#?}", &result);

    Ok(())
}

fn read_file(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn read_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = serde_yaml::from_str(&contents)?;
    Ok(config)
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


#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    from: Nacos,
    to: Nacos,
    ignore: Vec<IgnoreItem>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Nacos {
    addr: String,
    namespace: String,
    username: Option<String>,
    password: Option<String>,
    app_name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct IgnoreItem {
    data_id: String,
    fields: HashMap<String, Value>,
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
