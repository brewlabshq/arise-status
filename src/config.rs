use anyhow::{Context, Error};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcServer {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub ping_url: Option<String>,
    pub pagerduty_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub pagerduty_url: Option<String>,
    pub rpc_servers: Vec<RpcServer>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;
        
        let config: Config = serde_json::from_str(&content)
            .context("Failed to parse config file as JSON")?;
        
        if config.rpc_servers.is_empty() {
            return Err(Error::msg("Config must contain at least one RPC server"));
        }
        
        Ok(config)
    }
    
    pub fn get_pagerduty_url(&self) -> String {
        self.pagerduty_url
            .clone()
            .unwrap_or_else(|| "https://events.pagerduty.com/v2/enqueue".to_string())
    }
}

