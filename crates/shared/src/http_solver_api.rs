use anyhow::{anyhow, ensure, Context, Result};
use reqwest::header::HeaderValue;
use reqwest::{Client, Url};
use std::time::{Duration, Instant};

pub mod model;

/// Implements an abstract HTTP solver API, can be mocked, instrumented, etc.
#[mockall::automock]
#[async_trait::async_trait]
pub trait HttpSolverApi: Send + Sync {
    /// Submit a batch auction to the solver and wait for a solution.
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        deadline: Instant,
    ) -> Result<model::SettledBatchAuctionModel>;
}

/// Default implementation for HTTP solver API that uses the reqwest client.
pub struct DefaultHttpSolverApi {
    /// Name of this solver.
    ///
    /// Used for logging and metrics reporting purposes.
    pub name: &'static str,

    /// Network ID or name.
    ///
    /// Used for logging and metrics reporting purposes.
    pub network_name: String,

    /// Chain ID.
    ///
    /// Used for logging and metrics reporting purposes.
    pub chain_id: u64,

    /// Base solver url.
    pub base: Url,

    /// An async HTTP client instance that will be used to interact with the solver.
    pub client: Client,

    /// Other solver parameters.
    pub config: SolverConfig,
}

/// Configuration for solver requests.
#[derive(Debug, Default)]
pub struct SolverConfig {
    /// Optional value for the `X-API-KEY` header.
    pub api_key: Option<String>,

    /// Controls value of the `max_nr_exec_orders` parameter.
    pub max_nr_exec_orders: u32,

    /// Controls if we should fill the `ucp_policy` parameter.
    pub has_ucp_policy_parameter: bool,
}

#[async_trait::async_trait]
impl HttpSolverApi for DefaultHttpSolverApi {
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        deadline: Instant,
    ) -> Result<model::SettledBatchAuctionModel> {
        let timeout = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| anyhow!("no time left to send request"))?;
        // The timeout we give to the solver is one second less than the deadline to make up for
        // overhead from the network.
        let solver_timeout = timeout
            .checked_sub(Duration::from_secs(1))
            .ok_or_else(|| anyhow!("no time left to send request"))?;

        let mut url = self.base.clone();
        url.set_path("/solve");

        let instance_name = self.generate_instance_name();
        tracing::debug!("http solver instance name is {}", instance_name);

        url.query_pairs_mut()
            .append_pair("instance_name", &instance_name)
            .append_pair("time_limit", &solver_timeout.as_secs().to_string())
            .append_pair(
                "max_nr_exec_orders",
                self.config.max_nr_exec_orders.to_string().as_str(),
            );
        if self.config.has_ucp_policy_parameter {
            url.query_pairs_mut()
                .append_pair("ucp_policy", "EnforceForOrders");
        }

        let query = url.query().map(ToString::to_string).unwrap_or_default();
        let mut request = self.client.post(url).timeout(timeout);
        if let Some(api_key) = &self.config.api_key {
            let mut header = HeaderValue::from_str(api_key.as_str()).unwrap();
            header.set_sensitive(true);
            request = request.header("X-API-KEY", header);
        }
        let body = serde_json::to_string(&model).context("failed to encode body")?;
        tracing::trace!("request {}", body);
        let request = request.body(body.clone());
        let response = request.send().await.context("failed to send request")?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to decode response body")?;
        tracing::trace!("response {}", text);
        let context = || {
            format!(
                "request query {}, request body {}, response body {}",
                query, body, text
            )
        };
        ensure!(
            status.is_success(),
            "solver response is not success: status {}, {}",
            status,
            context()
        );
        serde_json::from_str(text.as_str())
            .with_context(|| format!("failed to decode response json, {}", context()))
    }
}

impl DefaultHttpSolverApi {
    fn generate_instance_name(&self) -> String {
        let now = chrono::Utc::now();
        format!(
            "{}_{}_{}",
            now.to_string(),
            self.network_name,
            self.chain_id
        )
        .chars()
        .map(|x| match x {
            ' ' => '_',
            '/' => '_',
            _ => x,
        })
        .collect()
    }
}
