use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub(crate) struct RpcResponse {
    jsonrpc: String,
    result: RpcResult,
    id: u32,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RpcResult {
    #[serde(rename = "feature-set")]
    feature_set: u64,
    #[serde(rename = "solana-core")]
    solana_core: String,
}

pub(crate) async fn check_version(rpc: String) -> Result<String, anyhow::Error> {
    let client = Client::new();

    let response = client
        .post(rpc)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getVersion"
        }))
        .send()
        .await?;

    let rpc_response: RpcResponse = response.json().await?;

    Ok(rpc_response.result.solana_core)
}
