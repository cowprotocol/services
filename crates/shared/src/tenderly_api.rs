//! Module containing Tenderly API implementation.

use {
    crate::{
        arguments::{display_option, display_secret_option},
        http_client::HttpClientFactory,
    },
    alloy::{
        primitives::{Address, B256, TxKind, U256, map::B256Map},
        rpc::types::{TransactionRequest, state::StateOverride as AlloyStateOverride},
    },
    anyhow::{Result, ensure},
    bytes_hex::BytesHex,
    clap::Parser,
    prometheus::IntGaugeVec,
    reqwest::{
        Url,
        header::{HeaderMap, HeaderValue},
    },
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        fmt::{self, Display, Formatter},
        sync::Arc,
    },
    thiserror::Error,
    tracing::instrument,
};
/// Trait for abstracting Tenderly API.
#[async_trait::async_trait]
pub trait TenderlyApi: Send + Sync + 'static {
    async fn simulate(&self, simulation: SimulationRequest) -> Result<SimulationResponse>;
    fn log(&self, simulation: SimulationRequest) -> Result<()>;
    fn simulation_url(&self, id: &str) -> Url;
}

const API_URL: &str = "https://api.tenderly.co";
const DASHBOARD_URL: &str = "https://dashboard.tenderly.co";

/// Tenderly HTTP API.
pub struct TenderlyHttpApi {
    api: Url,
    dashboard: Url,
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
            api: Url::parse(&format!(
                "{API_URL}/api/v1/account/{user}/project/{project}/"
            ))?,
            dashboard: Url::parse(&format!("{DASHBOARD_URL}/{user}/{project}/"))?,
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
    #[instrument(skip_all)]
    async fn simulate(&self, simulation: SimulationRequest) -> Result<SimulationResponse> {
        let url = crate::url::join(&self.api, "simulate");
        let body = serde_json::to_string(&simulation)?;

        let response = self
            .client
            .post(url)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;

        let ok = response.error_for_status_ref().map(|_| ());
        let status = response.status();
        let body = response.text().await?;
        // NOTE: Turn these logs on at your own risk... The Tenderly response
        // objects are huge (order of ~3M).
        tracing::trace!(status =% status.as_u16(), %body, "simulated");

        ok?;
        Ok(serde_json::from_str(&body)?)
    }

    fn log(&self, simulation: SimulationRequest) -> Result<()> {
        let request_url = crate::url::join(&self.api, "simulate");
        let simulation_url =
            crate::url::join(&self.dashboard, "simulator/$SIMULATION_ID").to_string();
        let body = serde_json::to_string(&simulation)?;

        #[rustfmt::skip]
        tracing::debug!(
            "resimulate by setting TENDERLY_API_KEY environment variable and running: \
            curl -X POST -H \"X-ACCESS-KEY: $TENDERLY_API_KEY\" -H \"Content-Type: application/json\" --data '{body}' {request_url} \
            | jq -r \".simulation.id\" \
            | read SIMULATION_ID; \
            echo {simulation_url} \
            | xargs xdg-open",
        );

        Ok(())
    }

    fn simulation_url(&self, id: &str) -> Url {
        crate::url::join(&self.dashboard, &format!("simulator/{id}"))
    }
}

/// Instrumented Tenderly HTTP API.
pub struct Instrumented {
    inner: TenderlyHttpApi,
    name: String,
}

#[async_trait::async_trait]
impl TenderlyApi for Instrumented {
    async fn simulate(&self, simulation: SimulationRequest) -> Result<SimulationResponse> {
        let result = self.inner.simulate(simulation).await;

        Metrics::get()
            .tenderly_simulations
            .with_label_values(&[
                &self.name,
                match &result {
                    Ok(_) => "ok",
                    Err(_) => "err",
                },
            ])
            .inc();

        result
    }

    fn log(&self, simulation: SimulationRequest) -> Result<()> {
        self.inner.log(simulation)
    }

    fn simulation_url(&self, id: &str) -> Url {
        self.inner.simulation_url(id)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimulationRequest {
    pub network_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<i64>,
    pub from: Address,
    pub to: Address,
    #[serde(with = "bytes_hex")]
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
    pub state_objects: Option<HashMap<Address, StateObject>>,
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
    pub balance: Option<alloy::primitives::U256>,

    /// Fake EVM bytecode to inject into the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<alloy::primitives::Bytes>,

    /// Fake key-value mapping to override **individual** slots in the account
    /// storage before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<B256Map<B256>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SimulationResponse {
    pub transaction: Transaction,
    pub generated_access_list: Option<Vec<AccessListItem>>,
    pub simulation: Simulation,
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

// Had to introduce copy of the web3 AccessList because tenderly responds with
// snake_case fields and tenderly storage_keys field does not exist if empty (it
// should be empty Vec instead)
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccessListItem {
    /// Accessed address
    pub address: Address,
    /// Accessed storage keys
    #[serde(default)]
    pub storage_keys: Vec<B256>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Simulation {
    pub id: String,
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

    /// Tenderly requires api key to work. Optional since Tenderly could be
    /// skipped in access lists estimators.
    #[clap(long, env)]
    pub tenderly_api_key: Option<String>,
}

impl Arguments {
    pub fn get_api_instance(
        &self,
        http_factory: &HttpClientFactory,
        name: String,
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
                    .map(|inner| Arc::new(Instrumented { inner, name }) as _),
                )
            })
            .transpose()
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            tenderly_user,
            tenderly_project,
            tenderly_api_key,
        } = self;

        display_option(f, "tenderly_user", tenderly_user)?;
        display_option(f, "tenderly_project", tenderly_project)?;
        display_secret_option(f, "tenderly_api_key", tenderly_api_key.as_ref())?;

        Ok(())
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Tenderly simulations.
    #[metric(labels("name", "result"))]
    tenderly_simulations: IntGaugeVec,
}

impl Metrics {
    fn get() -> &'static Metrics {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("simulation reverted {0:?}")]
    Revert(Option<String>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Clone for SimulationError {
    fn clone(&self) -> Self {
        match self {
            Self::Revert(message) => Self::Revert(message.clone()),
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

pub struct TenderlyCodeSimulator {
    tenderly: Arc<dyn TenderlyApi>,
    network_id: String,
}

impl TenderlyCodeSimulator {
    pub fn new(tenderly: Arc<dyn TenderlyApi>, network_id: impl ToString) -> Self {
        Self {
            tenderly,
            network_id: network_id.to_string(),
        }
    }

    fn prepare_request(
        &self,
        tx: TransactionRequest,
        overrides: AlloyStateOverride,
        block: Option<u64>,
    ) -> Result<SimulationRequest> {
        Ok(SimulationRequest {
            block_number: block,
            // By default, tenderly simulates on the top of the specified block, whereas regular
            // nodes simulate at the end of the specified block. This is to make
            // simulation results match in case critical state changed within the block.
            transaction_index: Some(-1),
            network_id: self.network_id.clone(),
            from: tx.from.unwrap_or_default(),
            to: tx.to.and_then(TxKind::into_to).unwrap_or_default(),
            input: tx.input.into_input().unwrap_or_default().to_vec(),
            gas: tx.gas,
            gas_price: tx
                .gas_price
                .map(TryInto::try_into)
                .map(|gas_price| gas_price.unwrap()),
            value: tx.value,
            simulation_kind: Some(SimulationKind::Quick),
            state_objects: Some(
                overrides
                    .into_iter()
                    .map(|(key, value)| Ok((key, value.try_into()?)))
                    .collect::<Result<_>>()?,
            ),
            ..Default::default()
        })
    }

    pub fn log_simulation_command(
        &self,
        tx: TransactionRequest,
        overrides: AlloyStateOverride,
        block: Option<u64>,
    ) -> Result<()> {
        let request = SimulationRequest {
            save: Some(true),
            save_if_fails: Some(true),
            ..self.prepare_request(tx, overrides, block)?
        };
        self.tenderly.log(request)
    }
}

impl TryFrom<alloy::rpc::types::eth::state::AccountOverride> for StateObject {
    type Error = anyhow::Error;

    fn try_from(
        value: alloy::rpc::types::eth::state::AccountOverride,
    ) -> std::result::Result<Self, Self::Error> {
        ensure!(
            value.nonce.is_none() && value.state.is_none(),
            "full state and nonce overrides not supported on Tenderly",
        );

        Ok(StateObject {
            balance: value.balance,
            code: value.code,
            storage: value.state_diff,
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::address,
        hex_literal::hex,
        serde_json::json,
        testlib::assert_json_matches,
    };

    #[test]
    fn serialize_deserialize_simulation_request() {
        let request = SimulationRequest {
            network_id: "1".to_string(),
            block_number: Some(14122310),
            from: address!("e92f359e6f05564849afa933ce8f62b8007a1d5d"),
            input: hex!("13d79a0b00000000000000000000000000000000000000000000").into(),
            to: address!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
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

        assert_json_matches!(serde_json::to_value(&request).unwrap(), json);
        assert_eq!(
            serde_json::from_value::<SimulationRequest>(json).unwrap(),
            request
        );
    }

    #[tokio::test]
    #[ignore]
    async fn simulate_transaction() {
        let tenderly = TenderlyHttpApi::test_from_env();
        let result = tenderly
            .simulate(SimulationRequest {
                network_id: "1".to_string(),
                to: address!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
                simulation_kind: Some(SimulationKind::Quick),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(result.transaction.status);
    }
}
