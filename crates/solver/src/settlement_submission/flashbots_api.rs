use super::submitter::{TransactionHandle, TransactionSubmitting};
use anyhow::{anyhow, bail, ensure, Context, Result};
use jsonrpc_core::Output;
use primitive_types::H256;
use reqwest::Client;
use serde::de::DeserializeOwned;

const URL: &str = "https://rpc.flashbots.net";

#[derive(Clone)]
pub struct FlashbotsApi {
    client: Client,
}

pub fn parse_json_rpc_response<T>(body: &str) -> Result<T>
//will be moved to CustomNodes impl in the following PR
where
    T: DeserializeOwned,
{
    serde_json::from_str::<Output>(body)
        .with_context(|| {
            tracing::info!("flashbot response: {}", body);
            anyhow!("invalid flashbots response")
        })
        .and_then(|output| match output {
            Output::Success(body) => serde_json::from_value::<T>(body.result).with_context(|| {
                format!(
                    "flashbots failed conversion to expected {}",
                    std::any::type_name::<T>()
                )
            }),
            Output::Failure(body) => bail!("flashbots rpc error: {}", body.error),
        })
}

impl FlashbotsApi {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl TransactionSubmitting for FlashbotsApi {
    async fn submit_raw_transaction(
        &self,
        raw_signed_transaction: &[u8],
    ) -> Result<TransactionHandle> {
        let tx = format!("0x{}", hex::encode(raw_signed_transaction));
        let body = serde_json::json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "eth_sendRawTransaction",
          "params": [tx],
        });
        tracing::debug!(
            "flashbots submit_transaction body: {}",
            serde_json::to_string(&body).unwrap_or_else(|err| format!("error: {:?}", err)),
        );
        let response = self.client.post(URL).json(&body).send().await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);
        tracing::debug!("flashbots submit response: {}", body);

        let bundle_id = parse_json_rpc_response::<H256>(&body)?;
        tracing::debug!("flashbots bundle id: {}", bundle_id);
        Ok(TransactionHandle(bundle_id))
    }

    async fn cancel_transaction(&self, _id: &TransactionHandle) -> Result<()> {
        Ok(())
    }
}
