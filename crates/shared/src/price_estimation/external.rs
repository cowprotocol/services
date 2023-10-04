use {
    super::{
        trade_finder::{TradeEstimator, TradeVerifier},
        PriceEstimateResult,
        PriceEstimating,
        Query,
    },
    crate::{rate_limiter::RateLimiter, trade_finding::external::ExternalTradeFinder},
    ethrpc::current_block::CurrentBlockStream,
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
    ) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ExternalTradeFinder::new(
                driver.clone(),
                client,
                block_stream,
            )),
            rate_limiter,
            driver.to_string(),
        ))
    }

    pub fn verified(&self, verifier: TradeVerifier) -> Self {
        Self(self.0.clone().with_verifier(verifier))
    }
}

impl PriceEstimating for ExternalPriceEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.0.estimate(query)
    }
}
