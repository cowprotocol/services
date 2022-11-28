//! Gelato HTTP API client and data types.
//!
//! <https://docs.gelato.network/developer-services/relay/quick-start/api>

use crate::http_client::HttpClientFactory;
use anyhow::{ensure, Result};
use chrono::{DateTime, Utc};
use derivative::Derivative;
use ethcontract::{H160, H256, U256};
use model::u256_decimal::DecimalU256;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, skip_serializing_none};
use std::fmt::{self, Display, Formatter};

pub struct GelatoClient {
    client: reqwest::Client,
    base: Url,
    api_key: String,
}

impl GelatoClient {
    pub fn new(factory: &HttpClientFactory, api_key: String) -> Self {
        Self::with_url(
            factory,
            Url::parse("https://relay.gelato.digital/").unwrap(),
            api_key,
        )
    }

    pub fn with_url(factory: &HttpClientFactory, base: Url, api_key: String) -> Self {
        Self {
            client: factory.create(),
            base,
            api_key,
        }
    }

    #[cfg(test)]
    pub fn test_from_env() -> Result<Self> {
        Ok(Self::new(
            &HttpClientFactory::default(),
            std::env::var("GELATO_API_KEY")?,
        ))
    }

    pub async fn sponsored_call(&self, call: &GelatoCall) -> Result<TaskId> {
        let response = self
            .client
            .post(self.base.join("relays/v2/sponsored-call")?)
            .json(&CallWithKey {
                call,
                sponsor_api_key: &self.api_key,
            })
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        ensure!(status.is_success(), "HTTP {status} error: {text}");

        let receipt = serde_json::from_str::<CallReceipt>(&text)?;
        Ok(receipt.task_id)
    }

    pub async fn task_status(&self, id: &TaskId) -> Result<Task> {
        let response = self
            .client
            .get(self.base.join("tasks/status/")?.join(&id.0)?)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        ensure!(status.is_success(), "HTTP {status} error: {text}");

        let data = serde_json::from_str::<TaskResponse>(&text)?;
        Ok(data.task)
    }
}

/// A call to send to the Gelato relay network and to be executed onchain.
#[serde_as]
#[skip_serializing_none]
#[derive(Clone, Derivative, Default, Serialize)]
#[derivative(Debug)]
#[serde(rename_all = "camelCase")]
pub struct GelatoCall {
    pub chain_id: u64,
    pub target: H160,
    #[derivative(Debug(format_with = "crate::debug_bytes"))]
    #[serde(with = "model::bytes_hex")]
    pub data: Vec<u8>,
    #[serde_as(as = "Option<DecimalU256>")]
    pub gas_limit: Option<U256>,
    pub retries: Option<usize>,
}

/// A task ID associated with a call that is queued for execution.
///
/// It looks like, from observing API responses that this is a 32-byte hash of
/// some sort. However, since this isn't documented behaviour - treat it like
/// an opaque string identifier.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct TaskId(String);

impl Display for TaskId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A task status.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub chain_id: u64,
    pub task_id: TaskId,
    pub task_state: TaskState,
    pub creation_date: DateTime<Utc>,
    pub last_check_date: Option<DateTime<Utc>>,
    pub last_check_message: Option<String>,
    pub transaction_hash: Option<H256>,
    pub execution_date: Option<DateTime<Utc>>,
    pub block_number: Option<u64>,
}

/// The state of a executing Gelato relay task.
///
/// <https://github.com/gelatodigital/relay-sdk/blob/c327debf611b606832dee9876c3f915d0262359b/src/lib/status/types/index.ts#L13-L22>
#[derive(Clone, Debug, Deserialize)]
pub enum TaskState {
    CheckPending,
    ExecPending,
    ExecSuccess,
    ExecReverted,
    WaitingForConfirmation,
    Blacklisted,
    Cancelled,
    NotFound,
}

/// Internal helper type for serializing a call with an API key.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CallWithKey<'a> {
    #[serde(flatten)]
    call: &'a GelatoCall,
    sponsor_api_key: &'a str,
}

/// Internal helper for deserializing call receipts.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallReceipt {
    task_id: TaskId,
}

/// Internal helper for deserializing task responses.
#[derive(Deserialize)]
struct TaskResponse {
    task: Task,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethrpc::{create_env_test_transport, Web3};
    use contracts::WETH9;
    use std::time::Duration;

    #[ignore]
    #[tokio::test]
    async fn execute_transaction() {
        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap();

        let weth = WETH9::deployed(&web3).await.unwrap();

        let gelato = GelatoClient::test_from_env().unwrap();
        let call = GelatoCall {
            chain_id: chain_id.as_u64(),
            target: weth.address(),
            data: weth.deposit().tx.data.unwrap().0,
            ..Default::default()
        };

        let id = gelato.sponsored_call(&call).await.unwrap();
        println!("executing task {id}");

        loop {
            let task = gelato.task_status(&id).await.unwrap();

            match task.task_state {
                TaskState::ExecSuccess => {
                    println!("task executed {:?}", task.transaction_hash.unwrap());
                    break;
                }
                TaskState::ExecReverted
                | TaskState::Blacklisted
                | TaskState::Cancelled
                | TaskState::NotFound => panic!("error executing {task:#?}"),
                state => {
                    println!("task is {state:?}...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
