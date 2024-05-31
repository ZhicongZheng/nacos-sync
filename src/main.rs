use std::{collections::HashMap, fs::File, hash::Hash, io::Read, str::FromStr};

use serde::{Deserialize, Serialize};

fn main() {
    let config = read_config("config.yaml").unwrap();
    print!("Config: {:#?}", config)
}

fn read_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = serde_yaml::from_str(&contents)?;
    Ok(config)
}

fn filter_ignore_config(config: HashMap<String, serde_yaml::Value>, ignore: &IgnoreItem) -> HashMap<String, serde_yaml::Value> {
    config
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
    fields: HashMap<String, serde_yaml::Value>,
}
