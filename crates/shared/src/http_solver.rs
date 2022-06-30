use anyhow::{anyhow, ensure, Context, Result};
use reqwest::header::{self, HeaderValue};
use reqwest::{Client, Url};
use std::time::Duration;

use crate::http_client::response_body_with_size_limit;
pub mod gas_model;
pub mod model;

const SOLVER_RESPONSE_SIZE_LIMIT: usize = 10_000_000;

/// Implements an abstract HTTP solver API, can be mocked, instrumented, etc.
#[mockall::automock]
#[async_trait::async_trait]
pub trait HttpSolverApi: Send + Sync {
    /// Submit a batch auction to the solver and wait for a solution.
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        timeout: Duration,
    ) -> Result<model::SettledBatchAuctionModel>;
}

/// Default implementation for HTTP solver API that uses the reqwest client.
pub struct DefaultHttpSolverApi {
    /// Name of this solver.
    ///
    /// Used for logging and metrics reporting purposes.
    pub name: String,

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
#[derive(Debug)]
pub struct SolverConfig {
    /// Optional value for the `X-API-KEY` header.
    pub api_key: Option<String>,

    /// Controls value of the `max_nr_exec_orders` parameter.
    pub max_nr_exec_orders: u32,

    /// Controls if we should fill the `ucp_policy` parameter.
    pub has_ucp_policy_parameter: bool,

    /// Controls if/how to set `use_internal_buffers`.
    pub use_internal_buffers: Option<bool>,

    /// Controls the objective function to optimize for.
    pub objective: Option<Objective>,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            max_nr_exec_orders: 100,
            has_ucp_policy_parameter: false,
            use_internal_buffers: None,
            objective: None,
        }
    }
}

#[derive(Debug)]
pub enum Objective {
    CappedSurplusFeesCosts,
    SurplusFeesCosts,
}

#[async_trait::async_trait]
impl HttpSolverApi for DefaultHttpSolverApi {
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        timeout: Duration,
    ) -> Result<model::SettledBatchAuctionModel> {
        // The timeout we give to the solver is one second less than
        // the deadline to make up for overhead from the network.
        // We use one second because the old MIP solver uses integer timeouts.
        let solver_timeout = timeout
            .checked_sub(Duration::from_secs(1))
            .ok_or_else(|| anyhow!("no time left to send request"))?;

        let mut url = self.base.clone();
        url.set_path("/solve");

        let maybe_auction_id = model.metadata.as_ref().and_then(|data| data.auction_id);
        let instance_name = self.generate_instance_name(maybe_auction_id.unwrap_or(0u64));
        tracing::debug!("http solver instance name is {}", instance_name);

        url.query_pairs_mut()
            .append_pair("instance_name", &instance_name)
            // Use integer remaining seconds for the time limit as the MIP solver
            // does not support fractional values here. Note that this means that
            // we don't have much granularity with the time limit.
            .append_pair("time_limit", &solver_timeout.as_secs().to_string())
            .append_pair(
                "max_nr_exec_orders",
                self.config.max_nr_exec_orders.to_string().as_str(),
            );
        if self.config.has_ucp_policy_parameter {
            url.query_pairs_mut()
                .append_pair("ucp_policy", "EnforceForOrders");
        }
        if let Some(use_internal_buffers) = self.config.use_internal_buffers {
            url.query_pairs_mut().append_pair(
                "use_internal_buffers",
                use_internal_buffers.to_string().as_str(),
            );
        }
        match self.config.objective {
            Some(Objective::CappedSurplusFeesCosts) => {
                url.query_pairs_mut()
                    .append_pair("objective", "cappedsurplusfeescosts");
            }
            Some(Objective::SurplusFeesCosts) => {
                url.query_pairs_mut()
                    .append_pair("objective", "surplusfeescosts");
            }
            _ => {}
        }
        if let Some(auction_id) = maybe_auction_id {
            url.query_pairs_mut()
                .append_pair("auction_id", auction_id.to_string().as_str());
        }
        let query = url.query().map(ToString::to_string).unwrap_or_default();
        let body = serde_json::to_string(&model).context("failed to encode body")?;
        tracing::trace!(%url, %body, "request");
        let mut request = self
            .client
            .post(url)
            .timeout(timeout)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json");
        if let Some(api_key) = &self.config.api_key {
            let mut header = HeaderValue::from_str(api_key.as_str()).unwrap();
            header.set_sensitive(true);
            request = request.header("X-API-KEY", header);
        }
        let request = request.body(body.clone());
        let mut response = request.send().await.context("failed to send request")?;
        let status = response.status();
        let response_body =
            response_body_with_size_limit(&mut response, SOLVER_RESPONSE_SIZE_LIMIT)
                .await
                .context("response body")?;
        let text = std::str::from_utf8(&response_body).context("failed to decode response body")?;
        tracing::trace!(body = %text, "response");
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

        serde_json::from_str(text)
            .with_context(|| format!("failed to decode response json, {}", context()))
    }
}

impl DefaultHttpSolverApi {
    fn generate_instance_name(&self, auction_id: u64) -> String {
        let now = chrono::Utc::now();
        format!(
            "{}_{}_{}_{}",
            now, self.network_name, self.chain_id, auction_id
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
