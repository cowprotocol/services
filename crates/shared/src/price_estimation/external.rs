use {
    super::{
        trade_finder::TradeEstimator,
        trade_verifier::TradeVerifying,
        PriceEstimateResult,
        PriceEstimating,
        Query,
    },
    crate::trade_finding::external::ExternalTradeFinder,
    ethrpc::block_stream::CurrentBlockStream,
    rate_limit::RateLimiter,
    reqwest::{Client, Url},
    std::sync::Arc,
};

pub struct ExternalPriceEstimator(TradeEstimator);

impl ExternalPriceEstimator {
    pub fn new(
        driver: Url,
        client: Client,
        rate_limiter: Arc<RateLimiter>,
        block_stream: CurrentBlockStream,
        timeout: std::time::Duration,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ExternalTradeFinder::new(
                driver.clone(),
                client,
                block_stream,
                timeout,
            )),
            rate_limiter,
            driver.to_string(),
        ))
    }

    pub fn verified(&self, verifier: Arc<dyn TradeVerifying>) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for ExternalPriceEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.0.estimate(query)
    }
}
