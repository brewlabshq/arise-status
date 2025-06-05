mod alive;
mod version;

use alive::handle_alive;
use anyhow::Error;
use dotenv::dotenv;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();
    let url = env::var("PING_URL").expect("PING_URL environment variable not set");
    let name = env::var("SERVICE_NAME").expect("SERVICE_NAME environment variable not set");
    let rpc = env::var("RPC_URL").expect("RPC_URL environment variable not set");
    let join_handle = handle_alive(rpc.clone(), url.clone(), name.clone()).unwrap();

    join_handle.await?;
    Ok(())
}
