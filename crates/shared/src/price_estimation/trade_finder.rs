//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use {
    super::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
        rate_limited,
        trade_verifier::{PriceQuery, TradeVerifying},
    },
    crate::trade_finding::{TradeError, TradeFinding},
    anyhow::{Result, anyhow},
    futures::future::FutureExt,
    rate_limit::RateLimiter,
    std::sync::Arc,
    tracing::instrument,
};

/// A `TradeFinding`-based price estimator with request sharing and rate
/// limiting.
#[derive(Clone)]
pub struct TradeEstimator {
    inner: Arc<Inner>,
    rate_limiter: Arc<RateLimiter>,
}

#[derive(Clone)]
struct Inner {
    finder: Arc<dyn TradeFinding>,
    // TODO: Make this required when verification is stable
    verifier: Option<Arc<dyn TradeVerifying>>,
}

impl TradeEstimator {
    pub fn new(finder: Arc<dyn TradeFinding>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            inner: Arc::new(Inner {
                finder,
                verifier: None,
            }),
            rate_limiter,
        }
    }

    pub fn with_verifier(mut self, verifier: Arc<dyn TradeVerifying>) -> Self {
        self.inner = Arc::new(Inner {
            verifier: Some(verifier),
            ..Arc::unwrap_or_clone(self.inner)
        });
        self
    }

    async fn estimate(&self, query: Arc<Query>) -> Result<Estimate, PriceEstimationError> {
        rate_limited(
            self.rate_limiter.clone(),
            self.inner.clone().estimate(query.clone()),
        )
        .await
    }
}

impl Inner {
    #[instrument(skip_all)]
    async fn estimate(
        self: Arc<Self>,
        query: Arc<Query>,
    ) -> Result<Estimate, PriceEstimationError> {
        if let Some(verifier) = &self.verifier {
            let trade = self.finder.get_trade(&query).await?;
            let price_query = PriceQuery {
                sell_token: query.sell_token,
                buy_token: query.buy_token,
                in_amount: query.in_amount,
                kind: query.kind,
            };

            return verifier
                .verify(&price_query, &query.verification, trade)
                .await
                .map_err(PriceEstimationError::EstimatorInternal);
        }

        let quote = self.finder.get_quote(&query).await?;
        Ok(Estimate {
            out_amount: quote.out_amount,
            gas: quote.gas_estimate,
            solver: quote.solver,
            verified: false,
            execution: quote.execution,
        })
    }
}

impl PriceEstimating for TradeEstimator {
    #[instrument(skip_all)]
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
