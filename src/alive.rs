// Use to check if server is alive or not
use anyhow::{Context, Error};
use reqwest::Client;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

const GET_SLOT_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"method":"getSlot","params":[{"commitment":"finalized"}]}"#;

/// Returns current slot (finalized) from RPC.
async fn get_slot(client: &Client, rpc_url: &str) -> Result<u64, Error> {
    let res = client
        .post(rpc_url)
        .body(GET_SLOT_BODY)
        .header("Content-Type", "application/json")
        .send()
        .await
        .context("getSlot request failed")?;
    let status = res.status();
    let body: serde_json::Value = res
        .json()
        .await
        .context("getSlot response not JSON")?;
    if !status.is_success() {
        anyhow::bail!("getSlot HTTP {}", status);
    }
    let slot = body
        .get("result")
        .and_then(|r| r.as_u64())
        .context("getSlot missing or non-numeric result")?;
    Ok(slot)
}

pub fn handle_alive(
    rpc: String,
    url: String,
    name: String,
    pagerduty_url: String,
    pagerduty_key: String,
    reference_rpc_url: Option<String>,
    slot_distance_threshold: u64,
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
                reference_rpc_url.clone(),
                slot_distance_threshold,
                &is_healthy_clone,
            )
            .await
            {
                Ok(_) => {}
                Err(err) => {
                    eprintln!("[{}] Error checking alive: {}", name, err);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
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
    reference_rpc_url: Option<String>,
    slot_distance_threshold: u64,
    is_healthy: &Arc<AtomicBool>,
) -> Result<(), Error> {
    // Create HTTP client with timeout configuration
    let client = Client::builder()
        .timeout(tokio::time::Duration::from_secs(10))
        .connect_timeout(tokio::time::Duration::from_secs(5))
        .build()?;

    let (healthy, error_msg) = if let Some(ref ref_url) = reference_rpc_url {
        // Slot-distance check: compare validator slot vs reference RPC slot
        let validator_slot = match get_slot(&client, &rpc).await {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("Validator getSlot failed: {}", e);
                eprintln!("[{}] {}", name, error_msg);
                handle_health_state_change(
                    false,
                    &name,
                    &rpc,
                    &pagerduty_url,
                    &pagerduty_key,
                    Some(error_msg.clone()),
                    is_healthy,
                )
                .await;
                return Err(e);
            }
        };
        let reference_slot = match get_slot(&client, ref_url).await {
            Ok(s) => s,
            Err(e) => {
                let error_msg = format!("Reference RPC getSlot failed: {}", e);
                eprintln!("[{}] {}", name, error_msg);
                handle_health_state_change(
                    false,
                    &name,
                    &rpc,
                    &pagerduty_url,
                    &pagerduty_key,
                    Some(error_msg.clone()),
                    is_healthy,
                )
                .await;
                return Err(e);
            }
        };
        let distance = (validator_slot as i64 - reference_slot as i64).unsigned_abs();
        let healthy = distance <= slot_distance_threshold;
        let error_msg = if healthy {
            None
        } else {
            Some(format!(
                "Slot distance {} > threshold {} (validator_slot={}, reference_slot={})",
                distance, slot_distance_threshold, validator_slot, reference_slot
            ))
        };
        if healthy {
            println!(
                "[{}] RPC alive (slot distance {} <= {})",
                name, distance, slot_distance_threshold
            );
        } else {
            eprintln!("[{}] {}", name, error_msg.as_deref().unwrap_or("slot distance exceeded"));
        }
        (healthy, error_msg)
    } else {
        // Fallback: /health endpoint check
        let health_url = format!("{}{}", rpc, "/health");
        let req = match client.get(&health_url).send().await {
            Ok(response) => response,
            Err(e) => {
                let error_msg = if e.is_timeout() {
                    format!("RPC server timeout - server may be down or unreachable: {}", e)
                } else if e.is_connect() {
                    format!("RPC server connection failed - server may be down: {}", e)
                } else {
                    format!("RPC server error - server may be down: {}", e)
                };
                eprintln!("[{}] RPC health check failed: {}", name, error_msg);
                handle_health_state_change(
                    false,
                    &name,
                    &rpc,
                    &pagerduty_url,
                    &pagerduty_key,
                    Some(error_msg.clone()),
                    is_healthy,
                )
                .await;
                return Err(Error::from(e));
            }
        };
        let is_success = req.status().is_success();
        if is_success {
            println!("[{}] RPC alive", name);
            (true, None)
        } else {
            let error_msg = format!("RPC returned status: {}", req.status());
            eprintln!("[{}] RPC health check failed: {}", name, error_msg);
            (false, Some(error_msg))
        }
    };

    handle_health_state_change(
        healthy,
        &name,
        &rpc,
        &pagerduty_url,
        &pagerduty_key,
        error_msg.clone(),
        is_healthy,
    )
    .await;

    if !healthy {
        return Err(Error::msg(
            error_msg.unwrap_or_else(|| "slot distance exceeded".to_string()),
        ));
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

    // Stable key so PagerDuty can match resolve to the same incident as trigger
    let dedup_key = format!(
        "arise-status-{}",
        service_name.replace(' ', "-").to_lowercase()
    );

    let payload = json!({
        "routing_key": pagerduty_key,
        "event_action": event_action,
        "dedup_key": dedup_key,
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
