//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use super::{
    rate_limited, Estimate, PriceEstimateResult, PriceEstimating, PriceEstimationError, Query,
};
use crate::{
    rate_limiter::RateLimiter,
    request_sharing::RequestSharing,
    trade_finding::{TradeError, TradeFinding},
};
use futures::{
    future::{BoxFuture, FutureExt as _},
    stream::StreamExt as _,
};
use std::sync::Arc;

/// A `TradeFinding`-based price estimator with request sharing and rate
/// limiting.
pub struct TradeEstimator {
    inner: Inner,
    sharing: RequestSharing<Query, BoxFuture<'static, Result<Estimate, PriceEstimationError>>>,
    rate_limiter: Arc<RateLimiter>,
}

#[derive(Clone)]
struct Inner {
    finder: Arc<dyn TradeFinding>,
}

impl TradeEstimator {
    pub fn new(finder: Arc<dyn TradeFinding>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            inner: Inner { finder },
            sharing: Default::default(),
            rate_limiter,
        }
    }

    async fn estimate(&self, query: Query) -> Result<Estimate, PriceEstimationError> {
        let estimate = rate_limited(
            self.rate_limiter.clone(),
            self.inner.clone().estimate(query),
        );
        self.sharing.shared(query, estimate.boxed()).await
    }
}

impl Inner {
    async fn estimate(self, query: Query) -> Result<Estimate, PriceEstimationError> {
        let trade = self.finder.get_trade(&query).await?;

        // TODO(nlordell): Here we can simulate and verify the trade calldata.

        Ok(Estimate {
            out_amount: trade.out_amount,
            gas: trade.gas_estimate,
        })
    }
}

impl PriceEstimating for TradeEstimator {
    fn estimates<'a>(
        &'a self,
        queries: &'a [Query],
    ) -> futures::stream::BoxStream<'_, (usize, PriceEstimateResult)> {
        debug_assert!(queries.iter().all(|query| {
            query.buy_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != model::order::BUY_ETH_ADDRESS
                && query.sell_token != query.buy_token
        }));

        futures::stream::iter(queries)
            .then(|query| self.estimate(*query))
            .enumerate()
            .boxed()
    }
}

impl From<TradeError> for PriceEstimationError {
    fn from(err: TradeError) -> Self {
        match err {
            TradeError::NoLiquidity => Self::NoLiquidity,
            TradeError::UnsupportedOrderType => Self::UnsupportedOrderType,
            TradeError::Other(err) => Self::Other(err),
        }
    }
}
