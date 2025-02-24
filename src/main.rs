use std::time::Duration;

use anyhow::Error;
use dotenv::dotenv;
use reqwest::Client;
use std::env;
use tokio_cron_scheduler::{Job, JobScheduler};

async fn post_status(url: &String) -> Result<(), Error> {
    let client = Client::new();

    let req = client.get(url.as_str()).send().await?;

    let is_success = req.status().is_success();
    if is_success {
        println!("Status posted successfully");
    } else {
        return Err(Error::msg("Failed to send status"));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    let sched = JobScheduler::new()
        .await
        .expect("Failed to create JobScheduler");
    let name = env::var("SERVICE_NAME").expect("SERVICE_NAME environment variable not set");
    println!("Starting Service for {}", name);
    // Create a job that calls post_status periodically.
    // Replace "http://example.com" with your actual URL.
    let job = match Job::new_repeated_async(Duration::from_secs(10), |_uuid, _lock| {
        Box::pin(async move {
            let url = env::var("PING_URL").expect("PING_URL environment variable not set");
            let name = env::var("SERVICE_NAME").expect("SERVICE_NAME environment variable not set");
            if let Err(err) = post_status(&url).await {
                eprintln!("Error posting status for {}: {}", name, err);
            }
        })
    }) {
        Ok(r) => r,
        Err(e) => return Err(Error::msg(e)),
    };

    sched.add(job).await.expect("Failed to add job");
    sched.start().await.expect("Failed to start scheduler");

    tokio::time::sleep(Duration::from_secs(100)).await;

    Ok(())
}
