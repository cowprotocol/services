//! A trade finder that uses an external driver.
use crate::{
    price_estimation::{
        rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
    },
    rate_limiter::RateLimiter,
    request_sharing::RequestSharing,
    trade_finding::{Quote, Trade, TradeError, TradeFinding},
};
use anyhow::Context;
use futures::{future::BoxFuture, stream::BoxStream, FutureExt, StreamExt};
use reqwest::{header, Client};
use std::sync::Arc;
use url::Url;

pub struct ExternalTradeFinder {
    /// URL to call to in the driver to get a quote with call data for a trade.
    quote_endpoint: Url,

    /// Utility to make sure no 2 identical requests are in-flight at the same time.
    /// Instead of issuing a duplicated request this awaits the response of the in-flight request.
    sharing: RequestSharing<Query, BoxFuture<'static, Result<Trade, TradeError>>>,

    /// Utility to temporarily drop requests when the driver responds too slowly to not slow down
    /// the whole price estimation logic.
    rate_limiter: Arc<RateLimiter>,

    /// Client to issue http requests with.
    client: Client,
}

impl ExternalTradeFinder {
    #[allow(dead_code)]
    pub fn new(driver: Url, client: Client, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            quote_endpoint: driver.join("/quote").unwrap(),
            sharing: Default::default(),
            rate_limiter,
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

#[async_trait::async_trait]
impl PriceEstimating for ExternalTradeFinder {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> BoxStream<'_, (usize, PriceEstimateResult)> {
        futures::stream::iter(queries)
            .then(|query| self.shared_query(query))
            .map(|result| match result {
                Ok(trade) => Ok(Estimate {
                    out_amount: trade.out_amount,
                    gas: trade.gas_estimate,
                }),
                Err(err) => Err(PriceEstimationError::from(err)),
            })
            .enumerate()
            .boxed()
    }
}
