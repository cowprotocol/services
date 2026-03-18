//! Module with shared logic for creating a `PriceEstimating` implementation
//! from an inner `TradeFinding`.

use {
    crate::{
        Estimate,
        PriceEstimateResult,
        PriceEstimating,
        PriceEstimationError,
        Query,
        rate_limited,
        trade_finding::{TradeError, TradeFinding},
        trade_verifier::{PriceQuery, TradeVerifying},
    },
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
            TradeError::TradingOutsideAllowedWindow { message } => {
                Self::TradingOutsideAllowedWindow { message }
            }
            TradeError::TokenTemporarilySuspended { message } => {
                Self::TokenTemporarilySuspended { message }
            }
            TradeError::InsufficientLiquidity { message } => {
                Self::InsufficientLiquidity { message }
            }
            TradeError::CustomSolverError { message } => Self::CustomSolverError { message },
            TradeError::Other(err) => Self::EstimatorInternal(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_custom_trade_errors_to_price_estimation_errors() {
        let cases = [
            (
                TradeError::TradingOutsideAllowedWindow {
                    message: "window".to_string(),
                },
                "window",
                0,
            ),
            (
                TradeError::TokenTemporarilySuspended {
                    message: "suspended".to_string(),
                },
                "suspended",
                1,
            ),
            (
                TradeError::InsufficientLiquidity {
                    message: "insufficient".to_string(),
                },
                "insufficient",
                2,
            ),
            (
                TradeError::CustomSolverError {
                    message: "custom".to_string(),
                },
                "custom",
                3,
            ),
        ];

        for (input, expected_message, expected_variant) in cases {
            let mapped: PriceEstimationError = input.into();
            match expected_variant {
                0 => assert!(matches!(
                    mapped,
                    PriceEstimationError::TradingOutsideAllowedWindow { message }
                    if message == expected_message
                )),
                1 => assert!(matches!(
                    mapped,
                    PriceEstimationError::TokenTemporarilySuspended { message }
                    if message == expected_message
                )),
                2 => assert!(matches!(
                    mapped,
                    PriceEstimationError::InsufficientLiquidity { message }
                    if message == expected_message
                )),
                3 => assert!(matches!(
                    mapped,
                    PriceEstimationError::CustomSolverError { message }
                    if message == expected_message
                )),
                _ => unreachable!(),
            }
        }
    }
}
