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
