mod alive;
mod version;

use alive::handle_alive;
use anyhow::Error;
use dotenv::dotenv;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    let url = env::var("PING_URL").unwrap_or_else(|_| String::new()); // Optional, can be empty
    let name = env::var("SERVICE_NAME").expect("SERVICE_NAME environment variable not set");
    let rpc = env::var("RPC_URL").expect("RPC_URL environment variable not set");
    
    // PagerDuty configuration
    let pagerduty_url = env::var("PAGERDUTY_URL")
        .unwrap_or_else(|_| "https://events.pagerduty.com/v2/enqueue".to_string());
    let pagerduty_key = env::var("PAGERDUTY_ROUTING_KEY")
        .expect("PAGERDUTY_ROUTING_KEY environment variable not set");
    
    println!("Starting ARISE Status Monitor");
    println!("Service: {}", name);
    println!("RPC URL: {}", rpc);
    println!("PagerDuty configured: {}", pagerduty_url);
    println!("Monitoring started - will alert on health check failures");
    
    let join_handle = handle_alive(
        rpc.clone(),
        url.clone(),
        name.clone(),
        pagerduty_url.clone(),
        pagerduty_key.clone(),
    )
    .unwrap();

    join_handle.await?;
    Ok(())
}
