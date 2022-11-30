//! Module containing Tenderly API implementation.

use crate::{
    arguments::{display_option, display_secret_option},
    http_client::HttpClientFactory,
};
use anyhow::Result;
use clap::Parser;
use model::bytes_hex::BytesHex;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Url,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    sync::Arc,
};
use web3::types::{Bytes, H160, H256, U256};

/// Trait for abstracting Tenderly API.
#[async_trait::async_trait]
pub trait TenderlyApi: Send + Sync + 'static {
    async fn block_number(&self, network_id: &str) -> Result<u64>;
    async fn simulate(&self, simulation: SimulationRequest) -> Result<SimulationResponse>;
}

const BASE_URL: &str = "https://api.tenderly.co/api";

/// Tenderly HTTP API.
pub struct TenderlyHttpApi {
    url: Url,
    client: reqwest::Client,
}

impl TenderlyHttpApi {
    /// Creates a new Tenderly API
    pub fn new(
        http_factory: &HttpClientFactory,
        user: &str,
        project: &str,
        api_key: &str,
    ) -> Result<Self> {
        let mut api_key = HeaderValue::from_str(api_key)?;
        api_key.set_sensitive(true);

        let mut headers = HeaderMap::new();
        headers.insert("x-access-key", api_key);

        Ok(Self {
            url: Url::parse(&format!("{BASE_URL}/v1/account/{user}/project/{project}/"))?,
            client: http_factory.configure(|builder| builder.default_headers(headers)),
        })
    }

    /// Creates a Tenderly API from the environment for testing.
    pub fn test_from_env() -> Arc<dyn TenderlyApi> {
        Arc::new(
            Self::new(
                &HttpClientFactory::default(),
                &std::env::var("TENDERLY_USER").unwrap(),
                &std::env::var("TENDERLY_PROJECT").unwrap(),
                &std::env::var("TENDERLY_API_KEY").unwrap(),
            )
            .unwrap(),
        )
    }
}

#[async_trait::async_trait]
impl TenderlyApi for TenderlyHttpApi {
    async fn block_number(&self, network_id: &str) -> Result<u64> {
        Ok(self
            .client
            .get(format!("{BASE_URL}/v1/network/{network_id}/block-number"))
            .send()
            .await?
            .error_for_status()?
            .json::<BlockNumber>()
            .await?
            .block_number)
    }

    async fn simulate(&self, simulation: SimulationRequest) -> Result<SimulationResponse> {
        Ok(self
            .client
            .post(self.url.join("simulate")?)
            .json(&simulation)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
}

#[derive(Deserialize)]
pub struct BlockNumber {
    pub block_number: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimulationRequest {
    pub network_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<u64>,
    pub from: H160,
    pub to: H160,
    #[serde(with = "model::bytes_hex")]
    pub input: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulation_kind: Option<SimulationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_if_fails: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_access_list: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_objects: Option<HashMap<H160, StateObject>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<AccessListItem>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimulationKind {
    Full,
    Quick,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct StateObject {
    /// Fake balance to set for the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<U256>,

    /// Fake EVM bytecode to inject into the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,

    /// Fake key-value mapping to override **individual** slots in the account
    /// storage before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<HashMap<H256, H256>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimulationResponse {
    pub transaction: Transaction,
    pub generated_access_list: Option<Vec<AccessListItem>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub status: bool,
    pub gas_used: u64,
    pub call_trace: Vec<CallTrace>,
}

#[serde_with::serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallTrace {
    #[serde(default)]
    #[serde_as(as = "Option<BytesHex>")]
    pub output: Option<Vec<u8>>,
    pub error: Option<String>,
}

// Had to introduce copy of the web3 AccessList because tenderly responds with snake_case fields
// and tenderly storage_keys field does not exist if empty (it should be empty Vec instead)
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccessListItem {
    /// Accessed address
    pub address: H160,
    /// Accessed storage keys
    #[serde(default)]
    pub storage_keys: Vec<H256>,
}

impl From<AccessListItem> for web3::types::AccessListItem {
    fn from(item: AccessListItem) -> Self {
        Self {
            address: item.address,
            storage_keys: item.storage_keys,
        }
    }
}

/// Tenderly API arguments.
#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// The Tenderly user associated with the API key.
    #[clap(long, env)]
    pub tenderly_user: Option<String>,

    /// The Tenderly project associated with the API key.
    #[clap(long, env)]
    pub tenderly_project: Option<String>,

    /// Tenderly requires api key to work. Optional since Tenderly could be skipped in access lists estimators.
    #[clap(long, env)]
    pub tenderly_api_key: Option<String>,
}

impl Arguments {
    pub fn get_api_instance(
        &self,
        http_factory: &HttpClientFactory,
    ) -> Result<Option<Arc<dyn TenderlyApi>>> {
        Some(())
            .and_then(|_| {
                Some(
                    TenderlyHttpApi::new(
                        http_factory,
                        self.tenderly_user.as_deref()?,
                        self.tenderly_project.as_deref()?,
                        self.tenderly_api_key.as_deref()?,
                    )
                    .map(|api| Arc::new(api) as _),
                )
            })
            .transpose()
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        display_option(f, "tenderly_user", &self.tenderly_user)?;
        display_option(f, "tenderly_project", &self.tenderly_project)?;
        display_secret_option(f, "tenderly_api_key", &self.tenderly_api_key)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;
    use serde_json::json;

    #[test]
    fn serialize_deserialize_simulation_request() {
        let request = SimulationRequest {
            network_id: "1".to_string(),
            block_number: Some(14122310),
            from: addr!("e92f359e6f05564849afa933ce8f62b8007a1d5d"),
            input: hex!("13d79a0b00000000000000000000000000000000000000000000").into(),
            to: addr!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
            generate_access_list: Some(true),
            transaction_index: None,
            gas: None,
            ..Default::default()
        };

        let json = json!({
            "network_id": "1",
            "block_number": 14122310,
            "from": "0xe92f359e6f05564849afa933ce8f62b8007a1d5d",
            "input": "0x13d79a0b00000000000000000000000000000000000000000000",
            "to": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
            "generate_access_list": true
        });

        assert_eq!(serde_json::to_value(&request).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<SimulationRequest>(json).unwrap(),
            request
        );
    }

    #[tokio::test]
    #[ignore]
    async fn get_block_number() {
        let tenderly = TenderlyHttpApi::test_from_env();
        let block_number = tenderly.block_number("1").await.unwrap();
        assert!(block_number > 0);
    }

    #[tokio::test]
    #[ignore]
    async fn simulate_transaction() {
        let tenderly = TenderlyHttpApi::test_from_env();
        let result = tenderly
            .simulate(SimulationRequest {
                network_id: "1".to_string(),
                to: addr!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
                simulation_kind: Some(SimulationKind::Quick),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(result.transaction.status);
    }
}
