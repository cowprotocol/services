use {
    super::{
        PriceEstimateResult,
        PriceEstimating,
        Query,
        trade_finder::TradeEstimator,
        trade_verifier::TradeVerifying,
    },
    crate::trade_finding::external::ExternalTradeFinder,
    ethrpc::block_stream::CurrentBlockWatcher,
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
        block_stream: CurrentBlockWatcher,
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ExternalTradeFinder::new(
                driver.clone(),
                client,
                block_stream,
            )),
            rate_limiter,
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
