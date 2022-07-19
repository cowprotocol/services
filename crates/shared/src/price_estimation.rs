pub mod balancer_sor;
pub mod baseline;
pub mod competition;
pub mod gas;
pub mod http;
pub mod instrumented;
pub mod native;
pub mod native_price_cache;
pub mod oneinch;
pub mod paraswap;
pub mod sanitized;
pub mod zeroex;

use crate::{
    bad_token::BadTokenDetecting,
    conversions::U256Ext,
    rate_limiter::{RateLimiter, RateLimiterError},
};
use anyhow::Result;
use ethcontract::{H160, U256};
use futures::{stream::BoxStream, StreamExt};
use model::order::OrderKind;
use num::BigRational;
use std::sync::Arc;
use std::{
    cmp::{Eq, PartialEq},
    future::Future,
    hash::Hash,
    time::{Duration, Instant},
};
use thiserror::Error;

#[derive(Copy, Clone, Debug, clap::ArgEnum, Hash, Eq, PartialEq)]
#[clap(rename_all = "verbatim")]
pub enum PriceEstimatorType {
    Baseline,
    Paraswap,
    ZeroEx,
    Quasimodo,
    OneInch,
    Yearn,
    BalancerSor,
}

impl PriceEstimatorType {
    /// Returns the name of this price estimator type.
    pub fn name(&self) -> String {
        format!("{:?}", self)
    }
}

#[derive(Error, Debug)]
pub enum PriceEstimationError {
    #[error("Token {0:?} not supported")]
    UnsupportedToken(H160),

    #[error("No liquidity")]
    NoLiquidity,

    #[error("Zero Amount")]
    ZeroAmount,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType,

    #[error(transparent)]
    Other(#[from] anyhow::Error),

    #[error(transparent)]
    RateLimited(#[from] RateLimiterError),
}

impl Clone for PriceEstimationError {
    fn clone(&self) -> Self {
        match self {
            Self::UnsupportedToken(token) => Self::UnsupportedToken(*token),
            Self::NoLiquidity => Self::NoLiquidity,
            Self::ZeroAmount => Self::ZeroAmount,
            Self::UnsupportedOrderType => Self::UnsupportedOrderType,
            Self::RateLimited(err) => Self::RateLimited(err.clone()),
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct Query {
    pub sell_token: H160,
    pub buy_token: H160,
    /// For OrderKind::Sell amount is in sell_token and for OrderKind::Buy in buy_token.
    pub in_amount: U256,
    pub kind: OrderKind,
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Estimate {
    pub out_amount: U256,
    /// full gas cost when settling this order alone on gp
    pub gas: u64,
}

impl Estimate {
    /// Returns (sell_amount, buy_amount).
    pub fn amounts(&self, query: &Query) -> (U256, U256) {
        match query.kind {
            OrderKind::Buy => (self.out_amount, query.in_amount),
            OrderKind::Sell => (query.in_amount, self.out_amount),
        }
    }

    /// The resulting price is how many units of sell_token needs to be sold for one unit of
    /// buy_token (sell_amount / buy_amount).
    pub fn price_in_sell_token_rational(&self, query: &Query) -> Option<BigRational> {
        let (sell_amount, buy_amount) = self.amounts(query);
        amounts_to_price(sell_amount, buy_amount)
    }

    /// The price for the estimate denominated in sell token.
    ///
    /// The resulting price is how many units of sell_token needs to be sold for one unit of
    /// buy_token (sell_amount / buy_amount).
    pub fn price_in_sell_token_f64(&self, query: &Query) -> f64 {
        let (sell_amount, buy_amount) = self.amounts(query);
        sell_amount.to_f64_lossy() / buy_amount.to_f64_lossy()
    }

    /// The price of the estimate denominated in buy token.
    ///
    /// The resulting price is how many units of buy_token are bought for one unit of
    /// sell_token (buy_amount / sell_amount).
    pub fn price_in_buy_token_f64(&self, query: &Query) -> f64 {
        let (sell_amount, buy_amount) = self.amounts(query);
        buy_amount.to_f64_lossy() / sell_amount.to_f64_lossy()
    }
}

pub type PriceEstimateResult = Result<Estimate, PriceEstimationError>;

#[mockall::automock]
pub trait PriceEstimating: Send + Sync + 'static {
    // The '_ lifetime in the return value is the same as 'a but we need to write it as underscore
    // because of a mockall limitation.

    /// Returns one result for each query in arbitrary order. The usize is the index into the queries slice.
    fn estimates<'a>(&'a self, queries: &'a [Query])
        -> BoxStream<'_, (usize, PriceEstimateResult)>;
}

/// Use a PriceEstimating with a single query.
pub async fn single_estimate(
    estimator: &dyn PriceEstimating,
    query: &Query,
) -> PriceEstimateResult {
    estimator
        .estimates(std::slice::from_ref(query))
        .next()
        .await
        .unwrap()
        .1
}

/// Use a streaming PriceEstimating with the old Vec based interface.
pub async fn vec_estimates(
    estimator: &dyn PriceEstimating,
    queries: &[Query],
) -> Vec<PriceEstimateResult> {
    let mut results = vec![None; queries.len()];
    let mut stream = estimator.estimates(queries);
    while let Some((index, result)) = stream.next().await {
        results[index] = Some(result);
    }
    let results = results.into_iter().flatten().collect::<Vec<_>>();
    // Check that every query has a result.
    debug_assert_eq!(results.len(), queries.len());
    results
}

/// Convert an old Vec based PriceEstimating implementation to a stream.
pub fn old_estimator_to_stream<'a, IntoIter>(
    estimator: impl Future<Output = IntoIter> + Send + 'a,
) -> BoxStream<'a, (usize, PriceEstimateResult)>
where
    IntoIter: IntoIterator<Item = PriceEstimateResult> + Send + 'a,
    IntoIter::IntoIter: Send + 'a,
{
    futures::stream::once(estimator)
        .flat_map(|iter| futures::stream::iter(iter.into_iter().enumerate()))
        .boxed()
}

pub async fn ensure_token_supported(
    token: H160,
    bad_token_detector: &dyn BadTokenDetecting,
) -> Result<(), PriceEstimationError> {
    match bad_token_detector.detect(token).await {
        Ok(quality) => {
            if quality.is_good() {
                Ok(())
            } else {
                Err(PriceEstimationError::UnsupportedToken(token))
            }
        }
        Err(err) => Err(PriceEstimationError::Other(err)),
    }
}

pub fn amounts_to_price(sell_amount: U256, buy_amount: U256) -> Option<BigRational> {
    if buy_amount.is_zero() {
        return None;
    }
    Some(BigRational::new(
        sell_amount.to_big_int(),
        buy_amount.to_big_int(),
    ))
}

const HEALTHY_PRICE_ESTIMATION_TIME: Duration = Duration::from_millis(5_000);

pub async fn rate_limited<T>(
    rate_limiter: Arc<RateLimiter>,
    estimation: impl Future<Output = Result<T, PriceEstimationError>>,
) -> Result<T, PriceEstimationError> {
    let timed_estimation = async move {
        let start = Instant::now();
        let result = estimation.await;
        (start.elapsed(), result)
    };
    let rate_limited_estimation = rate_limiter.execute(timed_estimation, |(estimation_time, _)| {
        *estimation_time > HEALTHY_PRICE_ESTIMATION_TIME
    });
    match rate_limited_estimation.await {
        Ok((_estimation_time, Ok(result))) => Ok(result),
        // return original PriceEstimationError
        Ok((_estimation_time, Err(err))) => Err(err),
        // convert the RateLimiterError to a PriceEstimationError
        Err(err) => Err(PriceEstimationError::from(err)),
    }
}

pub mod mocks {
    use super::*;
    use anyhow::anyhow;

    pub struct FakePriceEstimator(pub Estimate);
    impl PriceEstimating for FakePriceEstimator {
        fn estimates<'a>(
            &'a self,
            queries: &'a [Query],
        ) -> BoxStream<'_, (usize, PriceEstimateResult)> {
            futures::stream::iter((0..queries.len()).map(|i| (i, Ok(self.0)))).boxed()
        }
    }

    pub struct FailingPriceEstimator;
    impl PriceEstimating for FailingPriceEstimator {
        fn estimates<'a>(
            &'a self,
            queries: &'a [Query],
        ) -> BoxStream<'_, (usize, PriceEstimateResult)> {
            futures::stream::iter((0..queries.len()).map(|i| (i, Err(anyhow!("").into())))).boxed()
        }
    }
}
