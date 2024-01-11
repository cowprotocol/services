//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use {
    super::{
        rate_limited,
        trade_verifier::{NoopVerifier, PriceQuery, TradeVerifying},
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
    },
    crate::{
        request_sharing::RequestSharing,
        trade_finding::{TradeError, TradeFinding},
    },
    anyhow::{anyhow, Result},
    futures::future::{BoxFuture, FutureExt as _},
    rate_limit::RateLimiter,
    std::sync::Arc,
};

/// A `TradeFinding`-based price estimator with request sharing and rate
/// limiting.
pub struct TradeEstimator {
    inner: Arc<Inner>,
    sharing: RequestSharing<Arc<Query>, BoxFuture<'static, Result<Estimate, PriceEstimationError>>>,
    rate_limiter: Arc<RateLimiter>,
}

#[derive(Clone)]
struct Inner {
    finder: Arc<dyn TradeFinding>,
    verifier: Arc<dyn TradeVerifying>,
}

impl TradeEstimator {
    pub fn new(
        finder: Arc<dyn TradeFinding>,
        rate_limiter: Arc<RateLimiter>,
        label: String,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                finder,
                verifier: Arc::new(NoopVerifier),
            }),
            sharing: RequestSharing::labelled(format!("estimator_{}", label)),
            rate_limiter,
        }
    }

    pub fn with_verifier(mut self, verifier: Arc<dyn TradeVerifying>) -> Self {
        self.inner = Arc::new(Inner {
            verifier,
            ..arc_unwrap_or_clone(self.inner)
        });
        self
    }

    async fn estimate(&self, query: Arc<Query>) -> Result<Estimate, PriceEstimationError> {
        let estimate = rate_limited(
            self.rate_limiter.clone(),
            self.inner.clone().estimate(query.clone()),
        );
        self.sharing.shared(query, estimate.boxed()).await
    }
}

impl Inner {
    async fn estimate(
        self: Arc<Self>,
        query: Arc<Query>,
    ) -> Result<Estimate, PriceEstimationError> {
        match &query.verification {
            Some(verification) => {
                let trade = self.finder.get_trade(&query).await?;
                let price_query = PriceQuery {
                    sell_token: query.sell_token,
                    buy_token: query.buy_token,
                    in_amount: query.in_amount,
                    kind: query.kind,
                };
                let verified_quote = self
                    .verifier
                    .verify(&price_query, verification, trade.clone())
                    .await
                    .map_err(PriceEstimationError::EstimatorInternal);

                if let Err(err) = verified_quote {
                    tracing::warn!(?err, "failed verification; returning unverified estimate");
                    return Ok(Estimate {
                        out_amount: trade.out_amount,
                        gas: trade.gas_estimate,
                        solver: trade.solver,
                        verified: false,
                    });
                }

                verified_quote
            }
            None => {
                let quote = self.finder.get_quote(&query).await?;
                Ok(Estimate {
                    out_amount: quote.out_amount,
                    gas: quote.gas_estimate,
                    solver: quote.solver,
                    verified: false,
                })
            }
        }
    }
}

impl Clone for TradeEstimator {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            sharing: self.sharing.clone(),
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}

impl PriceEstimating for TradeEstimator {
    fn estimate(&self, query: Arc<Query>) -> futures::future::BoxFuture<'_, PriceEstimateResult> {
        self.estimate(query).boxed()
    }
}

impl From<TradeError> for PriceEstimationError {
    fn from(err: TradeError) -> Self {
        match err {
            TradeError::NoLiquidity => Self::NoLiquidity,
            TradeError::UnsupportedOrderType(order_type) => Self::UnsupportedOrderType(order_type),
            TradeError::DeadlineExceeded => Self::EstimatorInternal(anyhow!("timeout")),
            TradeError::RateLimited => Self::RateLimited,
            TradeError::Other(err) => Self::EstimatorInternal(err),
        }
    }
}

fn arc_unwrap_or_clone<T>(arc: Arc<T>) -> T
where
    T: Clone,
{
    Arc::try_unwrap(arc).unwrap_or_else(|arc| (*arc).clone())
}
