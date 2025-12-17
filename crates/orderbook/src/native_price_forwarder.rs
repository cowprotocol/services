use {
    alloy::primitives::Address,
    anyhow::Context,
    futures::{FutureExt, future::BoxFuture},
    model::quote::NativeTokenPrice,
    reqwest::StatusCode,
    shared::price_estimation::{
        PriceEstimationError,
        native::{NativePriceEstimateResult, NativePriceEstimating},
    },
    std::{sync::Arc, time::Duration},
    url::Url,
};

pub struct ForwardingNativePriceEstimator {
    client: reqwest::Client,
    autopilot_url: Url,
    fallback: Option<Arc<dyn NativePriceEstimating>>,
}

impl ForwardingNativePriceEstimator {
    pub fn new(
        client: reqwest::Client,
        autopilot_url: Url,
        fallback: Option<Arc<dyn NativePriceEstimating>>,
    ) -> Self {
        Self {
            client,
            autopilot_url,
            fallback,
        }
    }

    async fn try_fetch(&self, token: Address, timeout: Duration) -> NativePriceEstimateResult {
        let url = format!("{}/native_price/{:?}", self.autopilot_url, token);

        let response = self
            .client
            .get(&url)
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
            StatusCode::BAD_REQUEST => {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "unknown error".to_string());
                Err(PriceEstimationError::UnsupportedToken {
                    token,
                    reason: error_text,
                })
            }
            StatusCode::TOO_MANY_REQUESTS => Err(PriceEstimationError::RateLimited),
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

impl NativePriceEstimating for ForwardingNativePriceEstimator {
    fn estimate_native_price(
        &self,
        token: Address,
        timeout: Duration,
    ) -> BoxFuture<'_, NativePriceEstimateResult> {
        async move {
            match self.try_fetch(token, timeout).await {
                Ok(price) => Ok(price),
                Err(err) if should_fallback(&err) => {
                    tracing::warn!(?token, ?err, "autopilot request failed, using fallback");
                    if let Some(fallback) = &self.fallback {
                        fallback.estimate_native_price(token, timeout).await
                    } else {
                        Err(err)
                    }
                }
                Err(err) => Err(err),
            }
        }
        .boxed()
    }
}

fn should_fallback(err: &PriceEstimationError) -> bool {
    matches!(
        err,
        PriceEstimationError::ProtocolInternal(_) | PriceEstimationError::EstimatorInternal(_)
    )
}
