//! A trade finder that uses an external driver.
use crate::{
    price_estimation::{rate_limited, Query},
    rate_limiter::{RateLimiter, RateLimitingStrategy},
    request_sharing::RequestSharing,
    trade_finding::{Quote, Trade, TradeError, TradeFinding},
};
use anyhow::Context;
use futures::{future::BoxFuture, FutureExt};
use reqwest::{header, Client};
use std::sync::Arc;
use url::Url;

pub struct ExternalTradeFinder {
    quote_endpoint: Url,
    sharing: RequestSharing<Query, BoxFuture<'static, Result<Trade, TradeError>>>,
    rate_limiter: Arc<RateLimiter>,
    client: Client,
}

impl ExternalTradeFinder {
    #[allow(dead_code)]
    pub fn new(
        driver: Url,
        client: Client,
        name: String,
        rate_limiter: RateLimitingStrategy,
    ) -> Self {
        Self {
            quote_endpoint: driver.join("/quote").unwrap(),
            sharing: Default::default(),
            rate_limiter: Arc::new(RateLimiter::from_strategy(rate_limiter, name)),
            client,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes the result into a
    /// Quote or Trade.
    async fn shared_query(&self, query: &Query) -> Result<Trade, TradeError> {
        let body = serde_json::to_string(&query).context("failed to encode body")?;

        let request = self
            .client
            .post(self.quote_endpoint.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json")
            .body(body);

        let future = async {
            let response = request.send().await.map_err(TradeError::from)?;
            let text = response.text().await.map_err(TradeError::from)?;
            serde_json::from_str::<Trade>(&text).map_err(TradeError::from)
        };

        let future = rate_limited(self.rate_limiter.clone(), future);
        self.sharing
            .shared(*query, future.boxed())
            .await
            .map_err(TradeError::from)
    }
}

#[async_trait::async_trait]
impl TradeFinding for ExternalTradeFinder {
    async fn get_quote(&self, query: &Query) -> Result<Quote, TradeError> {
        // The driver only has a single endpoint to compute trades so we can simply reuse the same
        // logic here.
        let trade = self.get_trade(query).await?;
        Ok(Quote {
            out_amount: trade.out_amount,
            gas_estimate: trade.gas_estimate,
        })
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.shared_query(query).await
    }
}

impl From<reqwest::Error> for TradeError {
    fn from(error: reqwest::Error) -> Self {
        Self::Other(anyhow::anyhow!(error.to_string()))
    }
}

impl From<serde_json::Error> for TradeError {
    fn from(error: serde_json::Error) -> Self {
        Self::Other(anyhow::anyhow!(error.to_string()))
    }
}
