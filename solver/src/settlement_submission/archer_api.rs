//! https://docs.archerdao.io/for-traders/for-traders/traders

use anyhow::{ensure, Result};
use reqwest::Client;
use std::time::SystemTime;

const URL: &str = "https://api.archerdao.io/v1/transaction";

#[derive(Clone)]
pub struct ArcherApi {
    client: Client,
    authorization: String,
}

impl ArcherApi {
    pub fn new(authorization: String) -> Self {
        Self {
            client: Client::new(),
            authorization,
        }
    }

    /// Submit a signed transaction to the archer network.
    pub async fn submit_transaction(
        &self,
        raw_signed_transaction: &[u8],
        deadline: SystemTime,
    ) -> Result<()> {
        let tx = format!("0x{}", hex::encode(raw_signed_transaction));
        let deadline = deadline
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let body = serde_json::json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "archer_submitTx",
          "tx": tx,
          "deadline": deadline,
        });
        tracing::debug!(
            "archer submit_transaction body: {}",
            serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
        );
        let response = self
            .client
            .post(URL)
            .header("Authorization", &self.authorization)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);
        Ok(())
    }

    /// Cancel a previously submitted transaction.
    pub async fn cancel(&self, raw_signed_transaction: &[u8]) -> Result<()> {
        let tx = format!("0x{}", hex::encode(raw_signed_transaction));
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "archer_cancelTx",
            "tx": tx,
        });
        tracing::debug!(
            "archer_cancelTx body: {}",
            serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
        );
        let response = self
            .client
            .post(URL)
            .header("Authorization", &self.authorization)
            .json(&body)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);
        Ok(())
    }
}
