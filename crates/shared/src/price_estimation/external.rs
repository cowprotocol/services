use {
    super::{
        trade_finder::{TradeEstimator, TradeVerifier},
        PriceEstimateResult,
        PriceEstimating,
        Query,
    },
    crate::{rate_limiter::RateLimiter, trade_finding::external::ExternalTradeFinder},
    reqwest::{Client, Url},
    std::sync::Arc,
};

pub struct ExternalPriceEstimator(TradeEstimator);

impl ExternalPriceEstimator {
    pub fn new(driver: Url, client: Client, rate_limiter: Arc<RateLimiter>) -> Self {
        Self(TradeEstimator::new(
            Arc::new(ExternalTradeFinder::new(driver.clone(), client)),
            rate_limiter,
            driver.to_string(),
        ))
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
