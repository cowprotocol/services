//! A trade finder that uses an external driver.

use {
    crate::{
        price_estimation::{
            trade_finder::{TradeEstimator, TradeVerifier},
            PriceEstimateResult,
            PriceEstimating,
            PriceEstimationError,
            Query,
        },
        rate_limiter::RateLimiter,
        request_sharing::RequestSharing,
        trade_finding::{Quote, Trade, TradeError, TradeFinding},
    },
    anyhow::Context,
    futures::{future::BoxFuture, FutureExt},
    primitive_types::H160,
    reqwest::{header, Client},
    std::sync::Arc,
    url::Url,
};

pub struct ExternalPriceEstimator(TradeEstimator);

impl ExternalPriceEstimator {
    pub fn new(
        driver: Url,
        client: Client,
        rate_limiter: Arc<RateLimiter>,
        settlement: H160,
    ) -> Self {
        let trade_finder = Arc::new(ExternalTradeFinder::new(driver, client));
        Self(TradeEstimator::new(settlement, trade_finder, rate_limiter))
    }

    pub fn verified(&self, verifier: TradeVerifier) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for ExternalPriceEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        self.0.estimates(queries)
    }
}

#[derive(Clone)]
pub struct ExternalTradeFinder {
    /// URL to call to in the driver to get a quote with call data for a trade.
    quote_endpoint: Url,

    /// Utility to make sure no 2 identical requests are in-flight at the same
    /// time. Instead of issuing a duplicated request this awaits the
    /// response of the in-flight request.
    sharing: Arc<RequestSharing<Query, BoxFuture<'static, Result<Trade, PriceEstimationError>>>>,

    /// Client to issue http requests with.
    client: Client,
}

impl ExternalTradeFinder {
    #[allow(dead_code)]
    pub fn new(driver: Url, client: Client) -> Self {
        Self {
            quote_endpoint: driver.join("/quote").unwrap(),
            sharing: Default::default(),
            client,
        }
    }

    /// Queries the `/quote` endpoint of the configured driver and deserializes
    /// the result into a Quote or Trade.
    async fn shared_query(&self, query: &Query) -> Result<Trade, TradeError> {
        let body = serde_json::to_string(&query).context("failed to encode body")?;

        let request = self
            .client
            .post(self.quote_endpoint.clone())
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json")
            .body(body);

        let future = async {
            let response = request.send().await.map_err(PriceEstimationError::from)?;
            if response.status() == 429 {
                return Err(PriceEstimationError::RateLimited);
            }
            let text = response.text().await.map_err(PriceEstimationError::from)?;
            serde_json::from_str::<Trade>(&text).map_err(PriceEstimationError::from)
        };

        self.sharing
            .shared(*query, future.boxed())
            .await
            .map_err(TradeError::from)
    }
}

#[async_trait::async_trait]
impl TradeFinding for ExternalTradeFinder {
    async fn get_quote(&self, _query: &Query) -> Result<Quote, TradeError> {
        // TODO: this means we'll also not be able to use 0x, paraswap, 1inch to get
        // unverified quotes when switching to the co-located drivers.
        // This could be dealt with by returning the `gas_used` from the `/quote`
        // endpoint
        return Err(TradeError::Other(anyhow::anyhow!(
            "unverified quotes are unsupported for driver based price estimators"
        )));
    }

    async fn get_trade(&self, query: &Query) -> Result<Trade, TradeError> {
        self.shared_query(query).await
    }
}
