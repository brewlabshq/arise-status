use anyhow::{Context, Error};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcServer {
    pub name: String,
    /// Validator RPC URL (our node)
    pub url: String,
    #[serde(default)]
    pub ping_url: Option<String>,
    /// Stable reference RPC URL for slot comparison (e.g. Helius). If set, health = slot distance <= threshold.
    #[serde(default)]
    pub reference_rpc_url: Option<String>,
    /// Max allowed slot distance (validator vs reference). Alert when exceeded. Default 10.
    #[serde(default)]
    pub slot_distance_threshold: Option<u64>,
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

