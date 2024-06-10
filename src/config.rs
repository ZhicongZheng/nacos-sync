use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;

use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub from: Nacos,
    pub to: Nacos,
    pub ignore: Option<Vec<IgnoreItem>>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Nacos {
    pub addr: String,
    pub namespace: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub app_name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct IgnoreItem {
    pub data_id: String,
    pub fields: HashMap<String, Value>,
}

#[derive(Parser, Debug)]
#[command(
    name = "sync-sync_config", about = "Sync sync_config from one Nacos to another", version = "0.1.0", long_about = None
)]
pub struct Args {
    #[arg(short, long, help = "Path to the sync_config file")]
    pub config_path: Option<String>,
}

impl Config {
    pub fn read_config(path: &str) -> Result<Config, Box<dyn Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}

pub fn filter_config(
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
