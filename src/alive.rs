use std::fmt::format;

// Use to check if server is alive or not
use anyhow::Error;
use reqwest::Client;
use tokio::task::JoinHandle;

pub fn handle_alive(rpc: String, url: String, name: String) -> Result<JoinHandle<()>, Error> {
    let j = tokio::spawn(async move {
        loop {
            if let Err(err) = check_alive(rpc.clone(), url.clone(), name.clone()).await {
                eprintln!("Error checking alive for {:?}: {}", name, err);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    Ok(j)
}

pub(crate) async fn check_alive(rpc: String, url: String, name: String) -> Result<(), Error> {
    let client = Client::new();

    let req = client.get(format!("{}{}", rpc, "/health")).send().await?;

    let is_success = req.status().is_success();
    if is_success {
        println!("RPC alive");
    } else {
        return Err(Error::msg("RPC failed to respond"));
    }

    let client_ping = Client::new();

    let req_ping = client_ping.get(url.as_str()).send().await?;

    let is_success = req_ping.status().is_success();
    if is_success {
        println!("Status posted successfully");
    } else {
        eprintln!("Error posting status for {}", name);
    }
    Ok(())
}
