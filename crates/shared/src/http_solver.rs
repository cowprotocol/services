use {
    crate::http_client::response_body_with_size_limit,
    ::model::auction::AuctionId,
    anyhow::{anyhow, Context},
    reqwest::{
        header::{self, HeaderValue},
        Client,
        StatusCode,
        Url,
    },
    serde_json::json,
    std::time::Duration,
    tracing::Instrument,
};

pub mod gas_model;
pub mod model;

const SOLVER_RESPONSE_SIZE_LIMIT: usize = 10_000_000;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("rate limited")]
    RateLimited,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Implements an abstract HTTP solver API, can be mocked, instrumented, etc.
#[mockall::automock]
#[async_trait::async_trait]
pub trait HttpSolverApi: Send + Sync {
    /// Submit a batch auction to the solver and wait for a solution.
    async fn solve(
        &self,
        model: &model::BatchAuctionModel,
        timeout: Duration,
    ) -> Result<model::SettledBatchAuctionModel, Error>;

    /// Callback to notify the solver how it performed in the given auction (if
    /// it won or failed for some reason)
    fn notify_auction_result(&self, auction_id: AuctionId, result: model::AuctionResult);
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

    /// An async HTTP client instance that will be used to interact with the
    /// solver.
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
    ) -> Result<model::SettledBatchAuctionModel, Error> {
        // The timeout we give to the solver is one second less than
        // the deadline to make up for overhead from the network.
        // We use one second because the old MIP solver uses integer timeouts.
        let solver_timeout = timeout
            .checked_sub(Duration::from_secs(1))
            .context("no time left to send request")?;

        let mut url = self.base.join("solve").context("join base")?;

        let maybe_auction_id = model.metadata.as_ref().and_then(|data| data.auction_id);
        let instance_name = self.generate_instance_name(maybe_auction_id.unwrap_or(0));
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
        let context = || format!("request query {query}, response body {text}");
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited);
        }
        if !status.is_success() {
            return Err(anyhow!(
                "solver response is not success: status {}, {}",
                status,
                context()
            )
            .into());
        }
        serde_json::from_str(text)
            .with_context(|| format!("failed to decode response json, {}", context()))
            .map_err(Into::into)
    }

    fn notify_auction_result(&self, auction_id: AuctionId, result: model::AuctionResult) {
        let mut url = match self.base.join("notify") {
            Ok(url) => url,
            Err(err) => {
                tracing::error!(?err, "failed to create notify url");
                return;
            }
        };

        let client = self.client.clone();
        let config_api_key = self.config.api_key.clone();
        tracing::debug!(solver_name = self.name, ?result, "notify auction result");
        let future = async move {
            url.query_pairs_mut()
                .append_pair("auction_id", auction_id.to_string().as_str());

            let mut request = client
                .post(url)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ACCEPT, "application/json");

            if let Some(api_key) = config_api_key {
                let mut header = HeaderValue::from_str(api_key.as_str()).unwrap();
                header.set_sensitive(true);
                request = request.header("X-API-KEY", header);
            }

            let _result = request.json(&json!(result)).send().await;
        };
        tokio::task::spawn(future.instrument(tracing::Span::current()));
    }
}

impl DefaultHttpSolverApi {
    fn generate_instance_name(&self, auction_id: AuctionId) -> String {
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

#[cfg(test)]
mod tests {
    use {
        super::{model::SettledBatchAuctionModel, *},
        flate2::write::GzEncoder,
        tokio::{io::AsyncWriteExt, net::TcpListener},
    };

    #[tokio::test]
    async fn supports_gzip() {
        let listener = TcpListener::bind("localhost:1234").await.unwrap();
        let listen = async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut encoder = GzEncoder::new(Vec::new(), Default::default());
                serde_json::to_writer(&mut encoder, &SettledBatchAuctionModel::default()).unwrap();
                let body = encoder.finish().unwrap();
                let response = "\
HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\n\r\n";
                stream.write_all(response.as_bytes()).await.unwrap();
                stream.write_all(&body).await.unwrap();
                stream.shutdown().await.unwrap();
            }
        };
        tokio::task::spawn(listen);

        let mut api = DefaultHttpSolverApi {
            name: Default::default(),
            network_name: Default::default(),
            chain_id: Default::default(),
            base: "http://localhost:1234".parse().unwrap(),
            client: Default::default(),
            config: Default::default(),
        };
        // The default reqwest::Client supports gzip responses if the corresponding
        // crate feature is enabled.
        api.solve(&Default::default(), Duration::from_secs(1))
            .await
            .unwrap();
        // After explicitly disabling gzip support the response no longer decodes.
        api.client = reqwest::ClientBuilder::new().no_gzip().build().unwrap();
        api.solve(&Default::default(), Duration::from_secs(1))
            .await
            .unwrap_err();
    }
}
