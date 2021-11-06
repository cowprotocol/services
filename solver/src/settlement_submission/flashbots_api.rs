use anyhow::{anyhow, ensure, Result};
use reqwest::Client;

const URL: &str = "https://protection.flashbots.net/v1/rpc";

#[derive(Clone)]
pub struct FlashbotsApi {
    client: Client,
}

impl FlashbotsApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Submit a signed transaction to the flashbots protect network.
    pub async fn submit_transaction(&self, raw_signed_transaction: &[u8]) -> Result<String> {
        let params = format!("0x{}", hex::encode(raw_signed_transaction));
        let body = serde_json::json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "eth_sendRawTransaction",
          "params": [params],
        });
        tracing::debug!(
            "flashbots submit_transaction body: {}",
            serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
        );
        let response = self.client.post(URL).json(&body).send().await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);

        match serde_json::from_str::<jsonrpc_core::Output>(&body) {
            Ok(body) => {
                if let jsonrpc_core::Output::Success(ref x) = body {
                    match x.result.as_str() {
                        Some(result) => {
                            tracing::debug!(
                                "flashbots bundle id: {}",
                                serde_json::to_string(&result)
                                    .unwrap_or_else(|err| format!("error: {:?}", err)),
                            );
                            Ok(result.to_string())
                        }
                        None => Err(anyhow!("failed to submit: result not a string")),
                    }
                } else {
                    Err(anyhow!("failed to submit: response not success"))
                }
            }
            Err(err) => {
                tracing::debug!("failed to submit: {}", err);
                Err(anyhow!("failed to submit. Error: {}", err))
            }
        }
    }

    /// Cancel a previously submitted transaction.
    pub async fn cancel(&self, bundle_id: &str) -> Result<()> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_cancelBundleById",
            "params": [bundle_id],
        });
        tracing::debug!(
            "eth_cancelBundleById body: {}",
            serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
        );
        let response = self.client.post(URL).json(&body).send().await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);
        Ok(())
    }

    /// Query status of a previously submitted transaction.
    pub async fn status(&self, bundle_id: &str) -> Result<()> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBundleStatusById",
            "params": [bundle_id],
        });
        tracing::debug!(
            "eth_getBundleStatusById body: {}",
            serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
        );
        let response = self.client.post(URL).json(&body).send().await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);
        Ok(())
    }
}
