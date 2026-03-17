//! Module containing Tenderly API implementation.
use {
    crate::tenderly::dto,
    alloy_primitives::TxKind,
    alloy_rpc_types::{TransactionRequest, state::StateOverride as AlloyStateOverride},
    anyhow::{Result, ensure},
    http_client::HttpClientFactory,
    prometheus::IntGaugeVec,
    reqwest::{
        Url,
        header::{HeaderMap, HeaderValue},
    },
    std::sync::Arc,
    tracing::instrument,
};
/// Trait for abstracting Tenderly API.
#[async_trait::async_trait]
pub trait TenderlyApi: Send + Sync + 'static {
    async fn simulate(&self, simulation: dto::Request) -> Result<dto::Response>;
    fn log(&self, simulation: dto::Request) -> Result<()>;
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
    async fn simulate(&self, simulation: dto::Request) -> Result<dto::Response> {
        let url = crate::utils::join_url(&self.api, "simulate");
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

    fn log(&self, simulation: dto::Request) -> Result<()> {
        let request_url = crate::utils::join_url(&self.api, "simulate");
        let simulation_url =
            crate::utils::join_url(&self.dashboard, "simulator/$SIMULATION_ID").to_string();
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
        crate::utils::join_url(&self.dashboard, &format!("simulator/{id}"))
    }
}

/// Instrumented Tenderly HTTP API.
pub struct Instrumented {
    inner: TenderlyHttpApi,
    name: String,
}

#[async_trait::async_trait]
impl TenderlyApi for Instrumented {
    async fn simulate(&self, simulation: dto::Request) -> Result<dto::Response> {
        let result = self.inner.simulate(simulation).await;

        Metrics::get()
            .tenderly_simulations
            .with_label_values(&[
                self.name.as_str(),
                match &result {
                    Ok(_) => "ok",
                    Err(_) => "err",
                },
            ])
            .inc();

        result
    }

    fn log(&self, simulation: dto::Request) -> Result<()> {
        self.inner.log(simulation)
    }

    fn simulation_url(&self, id: &str) -> Url {
        self.inner.simulation_url(id)
    }
}

/*
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
}*/

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
    ) -> Result<dto::Request> {
        Ok(dto::Request {
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
            simulation_kind: Some(dto::SimulationKind::Quick),
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
        let request = dto::Request {
            save: Some(true),
            save_if_fails: Some(true),
            ..self.prepare_request(tx, overrides, block)?
        };
        self.tenderly.log(request)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::address,
        hex_literal::hex,
        serde_json::json,
        testlib::assert_json_matches,
    };

    #[test]
    fn serialize_deserialize_simulation_request() {
        let request = dto::Request {
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
            serde_json::from_value::<dto::Request>(json).unwrap(),
            request
        );
    }

    #[tokio::test]
    #[ignore]
    async fn simulate_transaction() {
        let tenderly = TenderlyHttpApi::test_from_env();
        let result = tenderly
            .simulate(dto::Request {
                network_id: "1".to_string(),
                to: address!("9008d19f58aabd9ed0d60971565aa8510560ab41"),
                simulation_kind: Some(dto::SimulationKind::Quick),
                ..Default::default()
            })
            .await
            .unwrap();

        assert!(result.transaction.status);
    }
}
