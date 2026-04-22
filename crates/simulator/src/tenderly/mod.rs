use {
    crate::ethereum::Ethereum,
    alloy_primitives::TxKind,
    alloy_rpc_types::{TransactionRequest, state::StateOverride},
    anyhow::{Result, anyhow},
    configs::simulator::TenderlyConfig,
    eth_domain_types::{AccessList, BlockNo, Gas},
    http_client::HttpClientFactory,
    prometheus::IntGaugeVec,
    reqwest::header::HeaderValue,
    thiserror::Error,
    url::Url,
};

pub mod dto;

const API_URL: &str = "https://api.tenderly.co/api";
const DASHBOARD_URL: &str = "https://dashboard.tenderly.co";
// We want the string to be printed together with a simulation so we
// don't care that it's not used for anything else.
#[derive(Debug)]
pub struct SimulationId(#[allow(dead_code, reason = "intended for Debug implementation")] String);

#[derive(Debug, Clone)]
pub struct Tenderly {
    api: TenderlyApi,
    eth: Ethereum,
    save: bool,
    save_if_fails: bool,
}

#[derive(Debug, Clone)]
pub struct TenderlyApi {
    /// Base URL for the Tenderly API project, e.g.
    /// `https://api.tenderly.co/api/v1/account/{user}/project/{project}`
    api_base: Url,
    client: reqwest::Client,
    dashboard: Url,
    chain_id: String,
}

#[async_trait::async_trait]
pub trait Api: Send + Sync + 'static {
    fn log_simulation_command(
        &self,
        tx: TransactionRequest,
        overrides: StateOverride,
        block: BlockNo,
    ) -> Result<()>;

    async fn simulate(&self, request: dto::Request) -> Result<dto::Response>;

    /// Submits a simulation, shares it, and returns the shared Tenderly URL.
    async fn simulate_and_share(&self, request: dto::Request) -> Result<String>;
}

impl Tenderly {
    pub fn new(config: &TenderlyConfig, eth: Ethereum, http_factory: &HttpClientFactory) -> Self {
        Self {
            api: TenderlyApi::new(config, http_factory, eth.chain().id().to_string()),
            eth,
            save_if_fails: config.save_if_fails,
            save: config.save,
        }
    }

    pub async fn simulate<T>(
        &self,
        tx: T,
        block: BlockNo,
        generate_access_list: GenerateAccessList,
    ) -> Result<Simulation, Error>
    where
        T: Into<TransactionRequest>,
    {
        let tx = tx.into();
        let request = dto::Request {
            generate_access_list: match generate_access_list {
                GenerateAccessList::Yes => Some(true),
                GenerateAccessList::No => None,
            },
            save_if_fails: self.save_if_fails.then_some(true),
            save: self.save.then_some(true),
            ..prepare_request(
                self.eth.chain().id().to_string(),
                &tx,
                Default::default(),
                block,
            )?
        };

        Ok(self
            .api
            .simulate(request)
            .await
            .map_err(|err| Error::Other(anyhow!(err)))?
            .into())
    }
}

impl TenderlyApi {
    pub fn new(
        config: &configs::simulator::TenderlyConfig,
        http_factory: &HttpClientFactory,
        chain_id: String,
    ) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut api_key =
            HeaderValue::from_str(&config.api_key).expect("api key is correct header value");
        api_key.set_sensitive(true);
        headers.insert("x-access-key", api_key);

        headers.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        Self {
            api_base: Url::parse(&format!(
                "{url}/v1/account/{user}/project/{project}/",
                url = config
                    .url
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| API_URL.to_owned()),
                user = config.user,
                project = config.project
            ))
            .expect("api url is valid Url"),
            dashboard: Url::parse(&format!(
                "{dashboard}/{user}/{project}/",
                dashboard = config
                    .dashboard
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| DASHBOARD_URL.to_owned()),
                user = config.user,
                project = config.project
            ))
            .expect("dashboard url is valid Url"),
            client: http_factory.configure(|builder| builder.default_headers(headers)),
            chain_id,
        }
    }

    pub fn new_instrumented(
        name: String,
        config: &configs::simulator::TenderlyConfig,
        http_factory: &HttpClientFactory,
        chain_id: String,
    ) -> Instrumented {
        Instrumented {
            inner: Self::new(config, http_factory, chain_id),
            name,
        }
    }
}

#[async_trait::async_trait]
impl Api for TenderlyApi {
    fn log_simulation_command(
        &self,
        tx: TransactionRequest,
        overrides: StateOverride,
        block: BlockNo,
    ) -> Result<()> {
        let request = dto::Request {
            save: Some(true),
            save_if_fails: Some(true),
            ..prepare_request(self.chain_id.clone(), &tx, overrides, block)?
        };
        let simulate_url = crate::utils::join_url(&self.api_base, "simulate");
        log_simulation_request(&simulate_url, &self.dashboard, request)
    }

    async fn simulate(&self, request: dto::Request) -> Result<dto::Response> {
        let body = serde_json::to_string(&request).map_err(|err| Error::Other(anyhow!(err)))?;

        let simulate_url = crate::utils::join_url(&self.api_base, "simulate");
        let response = self.client.post(simulate_url).body(body).send().await?;

        let ok = response.error_for_status_ref().map(|_| ());
        let status = response.status();
        let body = response.text().await?;
        // NOTE: Turn these logs on at your own risk... The Tenderly response
        // objects are huge (order of ~3M).
        tracing::trace!(status =% status.as_u16(), %body, "simulated");

        ok?;

        Ok(serde_json::from_str::<dto::Response>(&body)?)
    }

    async fn simulate_and_share(&self, request: dto::Request) -> Result<String> {
        let response = self.simulate(request).await?;
        let id = &response.simulation.id;
        self.share_simulation(id).await?;
        Ok(shared_simulation_url(id))
    }
}

impl TenderlyApi {
    async fn share_simulation(&self, id: &str) -> Result<()> {
        let url = crate::utils::join_url(&self.api_base, &format!("simulations/{id}/share"));
        self.client.post(url).send().await?.error_for_status()?;
        Ok(())
    }
}

fn shared_simulation_url(id: &str) -> String {
    format!("{DASHBOARD_URL}/shared/simulation/{id}")
}

pub fn prepare_request(
    chain_id: String,
    tx: &TransactionRequest,
    overrides: StateOverride,
    block: BlockNo,
) -> Result<dto::Request, Error> {
    Ok(dto::Request {
        block_number: Some(block.0),
        // By default, tenderly simulates on the top of the specified block, whereas regular
        // nodes simulate at the end of the specified block. This is to make
        // simulation results match in case critical state changed within the block.
        transaction_index: Some(-1),
        network_id: chain_id,
        from: tx.from.unwrap_or_default(),
        to: tx.to.and_then(TxKind::into_to).unwrap_or_default(),
        input: tx.input.clone().into_input().unwrap_or_default().to_vec(),
        gas: tx.gas,
        gas_price: tx
            .gas_price
            .map(TryInto::try_into)
            .map(|gas_price| gas_price.unwrap()),
        value: tx.value,
        simulation_type: Some(dto::SimulationType::Full),
        state_objects: Some(
            overrides
                .into_iter()
                .map(|(key, value)| Ok((key, value.try_into()?)))
                .collect::<Result<_>>()?,
        ),
        access_list: tx.access_list.as_ref().map(Into::into),
        ..Default::default()
    })
}

pub fn log_simulation_request(
    simulation_endpoint: &Url,
    dashboard: &Url,
    simulation: dto::Request,
) -> Result<()> {
    let simulation_url = crate::utils::join_url(dashboard, "simulator/$SIMULATION_ID").to_string();
    let body = serde_json::to_string(&simulation)?;

    #[rustfmt::skip]
    tracing::debug!(
        "resimulate by setting TENDERLY_API_KEY environment variable and running: \
        curl -X POST -H \"X-ACCESS-KEY: $TENDERLY_API_KEY\" -H \"Content-Type: application/json\" --data '{body}' {simulation_endpoint} \
        | jq -r \".simulation.id\" \
        | read SIMULATION_ID; \
        echo {simulation_url} \
        | xargs xdg-open",
        simulation_url = simulation_url
    );

    Ok(())
}

#[derive(Debug)]
pub struct Simulation {
    pub id: SimulationId,
    pub gas: Gas,
    pub access_list: AccessList,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GenerateAccessList {
    Yes,
    No,
}

impl From<dto::Response> for Simulation {
    fn from(value: dto::Response) -> Self {
        Simulation {
            id: SimulationId(value.simulation.id),
            gas: value.transaction.gas_used.into(),
            access_list: value.generated_access_list.unwrap_or_default().into(),
        }
    }
}

/// Instrumented Tenderly HTTP API.
pub struct Instrumented {
    inner: TenderlyApi,
    name: String,
}

#[async_trait::async_trait]
impl Api for Instrumented {
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

    fn log_simulation_command(
        &self,
        tx: TransactionRequest,
        overrides: StateOverride,
        block: BlockNo,
    ) -> Result<()> {
        self.inner.log_simulation_command(tx, overrides, block)
    }

    async fn simulate_and_share(&self, request: dto::Request) -> Result<String> {
        self.inner.simulate_and_share(request).await
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
#[error("tenderly error")]
pub enum Error {
    Http(#[from] reqwest::Error),
    Revert(SimulationId),
    Other(#[from] anyhow::Error),
}
