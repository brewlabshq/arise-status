mod alive;
mod config;
mod version;

use alive::handle_alive;
use anyhow::{Context, Error};
use config::Config;
use dotenv::dotenv;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    
    // Get config file path from environment or use default
    let config_path = env::var("CONFIG_FILE")
        .unwrap_or_else(|_| "config.json".to_string());
    
    let config = Config::from_file(&config_path)
        .with_context(|| format!("Failed to load config from: {}", config_path))?;
    
    let pagerduty_url = config.get_pagerduty_url();
    let interval_secs = config.get_interval_secs();

    println!("Starting ARISE Status Monitor");
    println!("Config file: {}", config_path);
    println!("PagerDuty URL: {}", pagerduty_url);
    println!("Monitoring {} RPC server(s)", config.rpc_servers.len());
    println!("---");
    
    // Spawn monitoring task for each RPC server
    let mut join_handles = Vec::new();
    
    for rpc_server in config.rpc_servers {
        let name = rpc_server.name.clone();
        let url = rpc_server.url.clone();
        let ping_url = rpc_server.ping_url.clone().unwrap_or_else(String::new);
        let pagerduty_key = rpc_server.pagerduty_key.clone();
        let pagerduty_url_clone = pagerduty_url.clone();
        let reference_rpc_url = rpc_server.reference_rpc_url.clone();
        let slot_distance_threshold = rpc_server.slot_distance_threshold.unwrap_or(10);
        let health_retry_count = rpc_server.health_retry_count.unwrap_or(3);
        let slot_behind_retry_count = rpc_server.slot_behind_retry_count.unwrap_or(1);

        println!("Starting monitor for: {} ({})", name, url);

        let handle = handle_alive(
            url,
            ping_url,
            name,
            pagerduty_url_clone,
            pagerduty_key,
            reference_rpc_url,
            slot_distance_threshold,
            health_retry_count,
            slot_behind_retry_count,
            interval_secs,
        )
        .context("Failed to start monitoring task")?;
        
        join_handles.push(handle);
    }
    
    println!("---");
    println!("All monitors started - will alert on health check failures");
    
    // Wait for all monitoring tasks (they run forever, so this will block)
    // In practice, you might want to handle graceful shutdown
    futures::future::join_all(join_handles).await;
    
    Ok(())
}
