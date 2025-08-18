//! Liquorice HTTP API client implementation.
//!
//! For more information on the HTTP API, consult:
//! <https://liquorice.gitbook.io/liquorice-docs>

use {
    anyhow::{Context, Result},
    reqwest::{
        Client,
        ClientBuilder,
        IntoUrl,
        Url,
        header::{HeaderMap, HeaderValue},
    },
    serde::{Deserialize, Serialize},
    std::{collections::HashSet, time::Duration},
    thiserror::Error,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeforeSettleNotification {
    /// UUIDs of Liquorice RFQs
    pub rfq_ids: HashSet<String>,
}

/// Liquorice API notify query parameters.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", content = "content", rename_all = "snake_case")]
pub enum NotifyQuery {
    /// Notify Liquorice before settlement.
    BeforeSettle(BeforeSettleNotification),
}

/// Abstract Liquorice API. Provides a mockable implementation.
#[async_trait::async_trait]
#[cfg(test)]
#[mockall::automock]
pub trait LiquoriceApi: Send + Sync {
    /// Sends notification to Liquorice API.
    async fn notify(&self, query: &NotifyQuery) -> Result<(), LiquoriceResponseError>;
}

/// Liquorice API Client implementation.
#[derive(Debug)]
pub struct DefaultLiquoriceApi {
    client: Client,
    base_url: Url,
}

impl DefaultLiquoriceApi {
    /// Default 0x API URL.
    pub const DEFAULT_URL: &'static str = "https://api.liquorice.tech/v1";

    /// Create a new 0x HTTP API client with the specified base URL.
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

    /// Create a Liquorice HTTP API client for testing using the default HTTP
    /// client.
    ///
    /// This method will attempt to read the `LIQUORICE_URL` (falling back to
    /// the default URL) and `LIQUORICE_API_KEY` (falling back to no API
    /// key) from the local environment when creating the API client.
    pub fn test() -> Self {
        Self::new(
            Client::builder(),
            std::env::var("LIQUORICE_URL").unwrap_or_else(|_| Self::DEFAULT_URL.to_string()),
            std::env::var("LIQUORICE_API_KEY").unwrap_or(String::new()),
            Duration::from_secs(1),
        )
        .unwrap()
    }

    /// Sends notification to Liquorice API
    pub async fn notify(&self, query: &NotifyQuery) -> Result<(), LiquoriceResponseError> {
        let url = format!("{}/{}", self.base_url, "notify");

        let _ = self
            .client
            .post(url)
            .json(query)
            .send()
            .await
            .map_err(|e| LiquoriceResponseError::Send(e))?
            .error_for_status()
            .map_err(|e| LiquoriceResponseError::Reqwest(e))?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum LiquoriceResponseError {
    // Connectivity or non-response error
    #[error("Failed on send")]
    Send(reqwest::Error),
    #[error("Reqwest error: {0}")]
    Reqwest(reqwest::Error),
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
