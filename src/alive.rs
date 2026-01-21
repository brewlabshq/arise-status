// Use to check if server is alive or not
use anyhow::Error;
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

pub fn handle_alive(
    rpc: String,
    url: String,
    name: String,
    pagerduty_url: String,
    pagerduty_key: String,
) -> Result<JoinHandle<()>, Error> {
    // Track the current health state per RPC instance to avoid duplicate alerts
    let is_healthy = Arc::new(AtomicBool::new(true));
    let is_healthy_clone = Arc::clone(&is_healthy);
    
    let j = tokio::spawn(async move {
        loop {
            match check_alive(
                rpc.clone(),
                url.clone(),
                name.clone(),
                pagerduty_url.clone(),
                pagerduty_key.clone(),
                &is_healthy_clone,
            )
            .await
            {
                Ok(_) => {
                    // Health check passed
                }
                Err(err) => {
                    eprintln!("[{}] Error checking alive: {}", name, err);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    Ok(j)
}

pub(crate) async fn check_alive(
    rpc: String,
    url: String,
    name: String,
    pagerduty_url: String,
    pagerduty_key: String,
    is_healthy: &Arc<AtomicBool>,
) -> Result<(), Error> {
    // Create HTTP client with timeout configuration
    // 5 second timeout for connection + 10 second timeout for total request
    let client = Client::builder()
        .timeout(tokio::time::Duration::from_secs(10))
        .connect_timeout(tokio::time::Duration::from_secs(5))
        .build()?;

    // Check RPC health
    let health_url = format!("{}{}", rpc, "/health");
    let req = match client.get(&health_url).send().await {
        Ok(response) => response,
        Err(e) => {
            // Handle various error scenarios:
            // - Server is down/unreachable
            // - Connection timeout
            // - DNS resolution failure
            // - Network errors
            let error_msg = if e.is_timeout() {
                format!("RPC server timeout - server may be down or unreachable: {}", e)
            } else if e.is_connect() {
                format!("RPC server connection failed - server may be down: {}", e)
            } else {
                format!("RPC server error - server may be down: {}", e)
            };
            
            eprintln!("[{}] RPC health check failed: {}", name, error_msg);
            handle_health_state_change(false, &name, &rpc, &pagerduty_url, &pagerduty_key, Some(error_msg), is_healthy).await;
            return Err(Error::from(e));
        }
    };

    let is_success = req.status().is_success();
    
    if is_success {
        println!("[{}] RPC alive", name);
        // Service is healthy - send resolve if it was previously unhealthy
        handle_health_state_change(true, &name, &rpc, &pagerduty_url, &pagerduty_key, None, is_healthy).await;
    } else {
        let status = req.status();
        let error_msg = format!("RPC returned status: {}", status);
        eprintln!("[{}] RPC health check failed: {}", name, error_msg);
        handle_health_state_change(false, &name, &rpc, &pagerduty_url, &pagerduty_key, Some(error_msg), is_healthy).await;
        return Err(Error::msg("RPC failed to respond"));
    }

    // Optionally keep the original ping URL functionality
    if !url.is_empty() {
        let client_ping = Client::new();
        if let Ok(req_ping) = client_ping.get(url.as_str()).send().await {
            if req_ping.status().is_success() {
                println!("[{}] Status posted successfully to ping URL", name);
            } else {
                eprintln!("[{}] Error posting status to ping URL", name);
            }
        }
    }

    Ok(())
}

async fn handle_health_state_change(
    is_healthy_now: bool,
    service_name: &str,
    rpc_url: &str,
    pagerduty_url: &str,
    pagerduty_key: &str,
    error_msg: Option<String>,
    is_healthy: &Arc<AtomicBool>,
) {
    let was_healthy = is_healthy.swap(is_healthy_now, Ordering::SeqCst);
    
    // Only send alerts on state transitions
    if was_healthy == is_healthy_now {
        return; // No state change, skip alert
    }

    let event_action = if is_healthy_now { "resolve" } else { "trigger" };
    let severity = if is_healthy_now { "info" } else { "critical" };
    
    let summary = if is_healthy_now {
        format!("Solana RPC health check recovered for {}", service_name)
    } else {
        format!("Solana RPC health check failed for {}", service_name)
    };

    let payload = json!({
        "routing_key": pagerduty_key,
        "event_action": event_action,
        "payload": {
            "summary": summary,
            "source": service_name,
            "severity": severity,
            "component": "solana-rpc",
            "custom_details": {
                "rpc_url": rpc_url,
                "service_name": service_name,
                "error": error_msg.unwrap_or_else(|| "Unknown error".to_string())
            }
        }
    });

    let client = Client::new();
    match client
        .post(pagerduty_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                println!("[{}] PagerDuty {} event sent successfully", service_name, event_action);
            } else {
                eprintln!("[{}] Failed to send PagerDuty event: HTTP {}", service_name, response.status());
            }
        }
        Err(e) => {
            eprintln!("[{}] Error sending PagerDuty event: {}", service_name, e);
        }
    }
}
