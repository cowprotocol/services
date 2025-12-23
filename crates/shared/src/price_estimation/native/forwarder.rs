//! Forwards native price estimation requests to autopilot's HTTP API.
//!
//! This allows orderbook instances to share autopilot's native price cache
//! instead of maintaining independent caches, avoiding cache inconsistencies
//! and reducing rate limiting from external price estimators.

use {
    super::{NativePriceEstimateResult, NativePriceEstimating},
    crate::price_estimation::PriceEstimationError,
    alloy::primitives::Address,
    anyhow::Context,
    futures::{FutureExt, future::BoxFuture},
    model::quote::NativeTokenPrice,
    reqwest::StatusCode,
    std::time::Duration,
    url::Url,
};

pub struct Forwarder {
    client: reqwest::Client,
    autopilot_url: Url,
}

impl Forwarder {
    pub fn new(client: reqwest::Client, autopilot_url: Url) -> Self {
        Self {
            client,
            autopilot_url,
        }
    }

    async fn try_fetch(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        let url = self
            .autopilot_url
            .join(format!("native_price/{:?}", token).as_str())
            .context("failed to construct autopilot URL")?;

        let response = self
            .client
            .get(url)
            .query(&[("timeout_ms", timeout.as_millis() as u64)])
            .timeout(timeout)
            .send()
            .await
            .context("failed to send request")?;

        match response.status() {
            StatusCode::OK => {
                let price: NativeTokenPrice =
                    response.json().await.context("failed to parse response")?;
                Ok(price.price)
            }
            StatusCode::NOT_FOUND => Err(PriceEstimationError::NoLiquidity),
            StatusCode::TOO_MANY_REQUESTS => Err(PriceEstimationError::RateLimited),
            StatusCode::BAD_REQUEST => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown error".to_string());
                Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                    "bad request: {}",
                    error_text
                )))
            }
            status => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| format!("HTTP {}", status));
                Err(PriceEstimationError::ProtocolInternal(anyhow::anyhow!(
                    "autopilot returned status {}: {}",
                    status,
                    error_text
                )))
            }
        }
    }
}

impl NativePriceEstimating for Forwarder {
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        self.try_fetch(token, timeout).boxed()
    }
}
