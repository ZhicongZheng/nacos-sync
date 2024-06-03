use std::{collections::HashMap, fs::File, io::Read};

use nacos_sdk::api::config::{ConfigService, ConfigServiceBuilder};
use nacos_sdk::api::props::ClientProps;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sync_config = read_config("sync-config.yaml").unwrap();

    let from = &sync_config.from;
    let from_config_service = build_config_service(from).unwrap();

    let to = &sync_config.to;
    let to_config_service = build_config_service(to).unwrap();

    let all_data_id = get_all_data_id(from).await?;
    println!("All Data ID: {:#?}", all_data_id);

    do_sync(all_data_id, &sync_config, Box::new(from_config_service), Box::new(to_config_service)).await?;

    Ok(())
}

fn build_config_service(nacos: &Nacos) -> Result<impl ConfigService, Box<dyn std::error::Error>> {
    let nacos_from = ClientProps::new()
        .server_addr(nacos.addr.clone())
        .namespace(nacos.namespace.clone())
        .auth_username(nacos.username.clone().unwrap())
        .auth_password(nacos.password.clone().unwrap());
    let config_service = ConfigServiceBuilder::new(nacos_from.clone())
        .enable_auth_plugin_http().build()?;
    Ok(config_service)
}

async fn do_sync(all_data_id: Vec<String>, sync_config: &Config, from_config_service: Box<dyn ConfigService>, to_config_service: Box<dyn ConfigService>) -> Result<(), Box<dyn std::error::Error>> {
    let ignore_vec = sync_config.ignore.clone();
    let map = ignore_vec.first().unwrap();
    let ignore: HashMap<String, Value> = map.fields.clone();
    println!("Ignore: {:#?}", &ignore);
    for data_id in all_data_id {
        let config_resp = from_config_service.get_config(data_id.clone(), "DEFAULT_GROUP".to_string()).await?;
        let contents = config_resp.content();
        let yaml_config: HashMap<String, Value> = serde_yaml::from_str(contents)?;

        let mut result = yaml_config;
        if ignore.contains_key(&data_id) {
            result = filter_config(&result, &ignore);
        }
        //println!("dataId: {}, Result: {:#?}", &data_id, &result);
        let sync_response = to_config_service.publish_config(data_id.clone(), "DEFAULT_GROUP".to_string(), serde_yaml::to_string(&result).unwrap(), Some("yaml".to_string())).await;
        match sync_response {
            Ok(_) => { println!("Sync Success: {}", &data_id) }
            Err(err) => { println!("Sync Failed: {}, Error: {:#?}", &data_id, &err) }
        }
    }
    Ok(())
}

async fn get_all_data_id(from_nacos: &Nacos) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // 创建一个 reqwest 客户端
    let client = reqwest::Client::new();
    let login_response = client.post(format!("http://{}/nacos/v1/auth/users/login?message=true", from_nacos.addr.clone()))
        .form(&[("username", from_nacos.username.clone().unwrap()), ("password", from_nacos.password.clone().unwrap())])
        .send()
        .await?;
    // 获取响应内容
    let body = login_response.text().await?;

    let token_json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let access_token = token_json.get("accessToken").unwrap().as_str().unwrap();

    let config_url = format!(
        "http://{}/nacos/v1/cs/configs?dataId=&group=&appName=&config_tags=&search=accurate&message=true&tenant={}&accessToken={}&pageNo=1&pageSize=10000",
        &from_nacos.addr, &from_nacos.namespace, &access_token);
    let config_list_response = client.get(config_url)
        .header("accessToken", access_token)
        .send()
        .await?;

    let config_list_responst_body = config_list_response.text().await?;
    let config_list_json: serde_json::Value = serde_json::from_str(&config_list_responst_body)?;

    let result = config_list_json
        .get("pageItems")
        .and_then(|items| items.as_array())
        .map(|items| {
            items.iter()
                .filter_map(|item| {
                    item.get("type")
                        .and_then(|t| t.as_str())
                        .filter(|&t| t == "yaml")
                        .and_then(|_| item.get("dataId"))
                        .and_then(|id| id.as_str())
                        .map(|id| id.to_string())
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    Ok(result)
}

// fn read_file(path: &str) -> Result<String, Box<dyn std::error::Error>> {
//     let mut file = File::open(path)?;
//     let mut contents = String::new();
//     file.read_to_string(&mut contents)?;
//     Ok(contents)
// }

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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
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
