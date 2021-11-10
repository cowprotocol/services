use anyhow::{anyhow, bail, ensure, Context, Result};
use gas_estimation::{EstimatedGasPrice, GasPrice1559};
use jsonrpc_core::Output;
use primitive_types::U256;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};
use std::time::Duration;

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

#[derive(Debug, Deserialize)]
struct FlashbotStatus {
    status: Status,
}

#[derive(Debug, Deserialize, PartialEq)]
enum Status {
    #[serde(rename = "PENDING_BUNDLE")]
    Pending,
    #[serde(rename = "FAILED_BUNDLE")]
    Failed,
    #[serde(rename = "SUCCESSFUL_BUNDLE")]
    Successful,
    #[serde(rename = "CANCEL_BUNDLE_SUCCESSFUL")]
    Cancelled,
}

fn parse_json_rpc_response<T>(body: &str) -> Result<T>
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

        let bundle_id = parse_json_rpc_response::<String>(&body)?;
        tracing::debug!("flashbots bundle id: {}", bundle_id);
        Ok(bundle_id)
    }

    /// Send a cancel to a previously submitted transaction. This function does not wait for cancellation result.
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

        let success = parse_json_rpc_response::<bool>(&body)?;
        tracing::debug!(
            "flashbots cancellation for bundle {} result: {}",
            bundle_id,
            success
        );

        match success {
            true => Ok(()),
            false => bail!("flashbots cancellation response was false"),
        }
    }

    /// Send cancel and wait for some time for the cancellation confirmal
    pub async fn cancel_and_wait(&self, bundle_id: &str) -> Result<bool> {
        self.cancel(bundle_id).await?;

        const WAIT_FOR_CANCELLATION_RETRIES: usize = 10usize; // will be a strategy parameter after the refactor!
        const CANCEL_PROPAGATION_TIME: Duration = Duration::from_secs(2); // will be a strategy parameter after the refactor!

        for _ in 0..std::cmp::max(WAIT_FOR_CANCELLATION_RETRIES, 1usize) {
            if self.status(bundle_id).await? == Status::Cancelled {
                return Ok(true);
            }

            tokio::time::sleep(CANCEL_PROPAGATION_TIME).await;
        }
        Ok(false)
    }

    /// Query status of a previously submitted transaction.
    async fn status(&self, bundle_id: &str) -> Result<Status> {
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

        // remove temporary status log in following refactor PR
        tracing::debug!("flashbot status response: {}", body);

        Ok(parse_json_rpc_response::<FlashbotStatus>(&body)?.status)
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

        let gas_price = parse_json_rpc_response::<FlashbotGasPrice>(&body)?;

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

        let deserialized = serde_json::from_str::<Output>(&body.to_string()).unwrap();
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

    #[test]
    fn deserialize_flashbot_status() {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "status": "FAILED_BUNDLE",
                "error": "EOA_MORE_THAN_ONE_BUNDLE",
                "message": "There is already a transaction being processed from that address",
                "id": "0x9ef2fec1c343354cacb62fb107cf330d3d3cc54345d5f30ba26ce36522b9ee3f"
            }
        });

        assert_eq!(
            parse_json_rpc_response::<FlashbotStatus>(&body.to_string())
                .unwrap()
                .status,
            Status::Failed
        );
    }
}
