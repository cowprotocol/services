//! Liquorice HTTP API client implementation.
//!
//! For more information on the HTTP API, consult:
//! <https://liquorice.gitbook.io/liquorice-docs>

pub mod request;

use {
    crate::infra::notify::liquidity_sources::liquorice::client::request::IsRequest,
    anyhow::{Context, Result},
    reqwest::{
        ClientBuilder,
        IntoUrl,
        Url,
        header::{HeaderMap, HeaderValue},
    },
    std::time::Duration,
};

/// Liquorice API Client implementation.
#[derive(Debug)]
pub struct Client {
    client: reqwest::Client,
    pub base_url: Url,
}

impl Client {
    /// Creates a new Liquorice HTTP API client with the specified API key and
    /// base URL.
    pub fn new(
        client_builder: ClientBuilder,
        base_url: impl IntoUrl,
        api_key: String,
        http_timeout: Duration,
    ) -> Result<Self> {
        let mut key = HeaderValue::from_str(&api_key)?;
        key.set_sensitive(true);

        let mut headers = HeaderMap::new();
        headers.insert("api-key", key);

        let client_builder = client_builder
            .default_headers(headers)
            .timeout(http_timeout);

        Ok(Self {
            client: client_builder
                .build()
                .context("failed to build reqwest client")?,
            base_url: base_url.into_url().context("liquorice api url")?,
        })
    }

    pub async fn send_request<R: IsRequest>(
        &self,
        request: R,
    ) -> Result<R::Response, request::Error> {
        request.send(&self.client, &self.base_url).await
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Counter for Liquorice API requests by URI path and result.
    #[metric(labels("path", "result"))]
    liquorice_api_requests: prometheus::IntCounterVec,
}

impl Metrics {
    fn _get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}
