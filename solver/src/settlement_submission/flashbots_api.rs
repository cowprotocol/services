use anyhow::{anyhow, bail, ensure, Context, Result};
use gas_estimation::{EstimatedGasPrice, GasPrice1559};
use jsonrpc_core::Output;
use primitive_types::U256;
use reqwest::Client;
use serde::Deserialize;

const URL: &str = "https://protection.flashbots.net/v1/rpc";

#[derive(Clone)]
pub struct FlashbotsApi {
    client: Client,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Eip1559 {
    max_fee_per_gas: U256,
    max_priority_fee_per_gas: U256,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlashbotGasPrice {
    base_fee_per_gas: U256,
    default: Eip1559,
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
          "method": "eth_sendRawTransactions",
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

        match serde_json::from_str::<Output>(&body) {
            Ok(body) => match body {
                Output::Success(body) => match body.result.as_str() {
                    Some(bundle_id) => {
                        tracing::debug!("flashbots bundle id: {}", bundle_id);
                        Ok(bundle_id.to_string())
                    }
                    None => Err(anyhow!("result not a string")),
                },
                Output::Failure(body) => Err(anyhow!(body.error)),
            },
            Err(err) => Err(anyhow!(err)),
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

        match serde_json::from_str::<Output>(&body) {
            Ok(body) => match body {
                Output::Success(body) => match body.result.as_bool() {
                    Some(success) => {
                        tracing::debug!("flashbots cancellation request sent: {}", success);
                        match success {
                            true => Ok(()),
                            false => Err(anyhow!("flashbots cancellation response was false")),
                        }
                    }
                    None => Err(anyhow!("result not a bool")),
                },
                Output::Failure(body) => Err(anyhow!(body.error)),
            },
            Err(err) => {
                tracing::info!("flashbot cancellation response: {}", body);
                Err(anyhow!(err))
            }
        }
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

    /// Query gas_price for the current network state (simplest one for Flashbots)
    pub async fn gas_price(&self) -> Result<EstimatedGasPrice> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_gasFees",
            "params": [],
        });
        let response = self.client.post(URL).json(&body).send().await?;
        let status = response.status();
        let body = response.text().await?;
        ensure!(status.is_success(), "status {}: {:?}", status, body);

        let gas_price = serde_json::from_str::<Output>(&body)
            .with_context(|| {
                tracing::info!("flashbot cancellation response: {}", body);
                anyhow!("invalid Flashbots RPC response")
            })
            .and_then(|output| match output {
                Output::Success(body) => serde_json::from_value::<FlashbotGasPrice>(body.result)
                    .context("result not a FlashbotGasPrice"),
                Output::Failure(body) => bail!("Flashbots RPC error: {}", body.error),
            })?;
        Ok(EstimatedGasPrice {
            eip1559: Some(GasPrice1559 {
                base_fee_per_gas: gas_price.base_fee_per_gas.to_f64_lossy(),
                max_fee_per_gas: gas_price.default.max_fee_per_gas.to_f64_lossy(),
                max_priority_fee_per_gas: gas_price.default.max_priority_fee_per_gas.to_f64_lossy(),
            }),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpc_core::Output;

    #[test]
    fn deserialize_flashbot_gas_price() {
        let body = serde_json::json!({
          "jsonrpc": "2.0",
          "id": 1,
          "result": {
              "block": 13575331,
              "baseFeePerGas": "0x10c38ad7b0",
              "default": {
                  "maxFeePerGas": "0x1ada38961e",
                  "maxPriorityFeePerGas": "0x02af6c0f03"
              },
              "low": {
                  "maxFeePerGas": "0x195113e77d",
                  "maxPriorityFeePerGas": "0x01440dcb93"
              },
              "med": {
                  "maxFeePerGas": "0x1ada38961e",
                  "maxPriorityFeePerGas": "0x02af6c0f03"
              },
              "high": {
                  "maxFeePerGas": "0x1c7b36646b",
                  "maxPriorityFeePerGas": "0x0445ae8f10"
              }
          },
        });

        let deserialized = serde_json::from_str::<jsonrpc_core::Output>(&body.to_string()).unwrap();
        match deserialized {
            Output::Success(s) => {
                let deserialized = serde_json::from_value::<FlashbotGasPrice>(s.result).unwrap();
                assert_eq!(
                    deserialized.default.max_fee_per_gas,
                    U256::from(115330291230u64)
                );
                assert_eq!(
                    deserialized.default.max_priority_fee_per_gas,
                    U256::from(11533029123u64)
                );
            }
            Output::Failure(_) => panic!(),
        }
    }
}
