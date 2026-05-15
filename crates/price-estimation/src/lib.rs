pub use configs::{
    native_price_estimators::{ExternalSolver, NativePriceEstimator, NativePriceEstimators},
    price_estimation::QuoteVerificationMode,
};
use {
    crate::trade_finding::QuoteExecution,
    alloy::primitives::{Address, U256},
    anyhow::Result,
    futures::future::BoxFuture,
    model::order::OrderKind,
    number::nonzero::NonZeroU256,
    rate_limit::RateLimiter,
    serde::{Deserialize, Serialize},
    std::{
        cmp::{Eq, PartialEq},
        future::Future,
        sync::Arc,
        time::{Duration, Instant},
    },
    thiserror::Error,
};

mod buffered;
pub mod competition;
pub mod config;
pub mod external;
pub mod factory;
pub mod gas;
pub mod instrumented;
pub mod native;
pub mod native_price_cache;
pub mod sanitized;
pub mod trade_finding;
pub mod trade_verifier;
pub mod utils;

#[derive(Error, Debug)]
pub enum PriceEstimationError {
    #[error("token {token:?} is not supported: {reason:}")]
    UnsupportedToken { token: Address, reason: String },

    #[error("No liquidity")]
    NoLiquidity,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType(String),

    #[error("Rate limited")]
    RateLimited,

    /// Token can only be traded during specific time windows (e.g. xStocks/Ondo
    /// RWA tokens).
    #[error("{message}")]
    TradingOutsideAllowedWindow { message: String },

    /// Token is temporarily suspended from trading by the solver.
    #[error("{message}")]
    TokenTemporarilySuspended { message: String },

    /// Insufficient liquidity to fill the requested trade size.
    #[error("{message}")]
    InsufficientLiquidity { message: String },

    /// Solver returned a custom error that doesn't map to a known variant.
    #[error("{message}")]
    CustomSolverError { message: String },

    #[error(transparent)]
    EstimatorInternal(anyhow::Error),

    #[error(transparent)]
    ProtocolInternal(anyhow::Error),
}

#[cfg(test)]
impl PartialEq for PriceEstimationError {
    // Can't use `Self` here because `discriminant` is only defined for enums
    // and the compiler is not smart enough to figure out that `Self` is always
    // an enum here.
    fn eq(&self, other: &PriceEstimationError) -> bool {
        let me = self as &PriceEstimationError;
        std::mem::discriminant(me) == std::mem::discriminant(other)
    }
}

impl Clone for PriceEstimationError {
    fn clone(&self) -> Self {
        match self {
            Self::UnsupportedToken { token, reason } => Self::UnsupportedToken {
                token: *token,
                reason: reason.clone(),
            },
            Self::NoLiquidity => Self::NoLiquidity,
            Self::UnsupportedOrderType(order_type) => {
                Self::UnsupportedOrderType(order_type.clone())
            }
            Self::RateLimited => Self::RateLimited,
            Self::TradingOutsideAllowedWindow { message } => Self::TradingOutsideAllowedWindow {
                message: message.clone(),
            },
            Self::TokenTemporarilySuspended { message } => Self::TokenTemporarilySuspended {
                message: message.clone(),
            },
            Self::InsufficientLiquidity { message } => Self::InsufficientLiquidity {
                message: message.clone(),
            },
            Self::CustomSolverError { message } => Self::CustomSolverError {
                message: message.clone(),
            },
            Self::EstimatorInternal(err) => {
                Self::EstimatorInternal(crate::utils::clone_anyhow_error(err))
            }
            Self::ProtocolInternal(err) => {
                Self::ProtocolInternal(crate::utils::clone_anyhow_error(err))
            }
        }
    }
}

#[cfg(test)]
mod price_estimation_error_tests {
    use super::PriceEstimationError;

    #[test]
    fn clone_preserves_custom_solver_error_messages() {
        let cases = [
            PriceEstimationError::TradingOutsideAllowedWindow {
                message: "window".to_string(),
            },
            PriceEstimationError::TokenTemporarilySuspended {
                message: "suspended".to_string(),
            },
            PriceEstimationError::InsufficientLiquidity {
                message: "insufficient".to_string(),
            },
            PriceEstimationError::CustomSolverError {
                message: "custom".to_string(),
            },
        ];

        for err in cases {
            let cloned = err.clone();
            assert_eq!(cloned.to_string(), err.to_string());
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Query {
    pub sell_token: Address,
    pub buy_token: Address,
    /// For OrderKind::Sell amount is in sell_token and for OrderKind::Buy in
    /// buy_token.
    pub in_amount: NonZeroU256,
    pub kind: OrderKind,
    pub verification: Verification,
    /// Signals whether responses from that were valid on previous blocks can be
    /// used to answer the query.
    #[serde(skip_serializing)]
    pub block_dependent: bool,
    pub timeout: Duration,
}

/// Conditions under which a given price estimate needs to work in order to be
/// viable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Verification {
    /// This address needs to have the `sell_token`.
    pub from: Address,
    /// This address will receive the `buy_token`.
    pub receiver: Address,
    /// App data provided with the quote that encodes things like
    /// hooks and custom wrappers.
    pub app_data: Arc<String>,
}

impl Verification {
    pub fn effective_receiver(&self) -> &Address {
        match self.receiver.is_zero() {
            true => &self.from,
            false => &self.receiver,
        }
    }
}

#[derive(Clone, derive_more::Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Estimate {
    pub out_amount: U256,
    /// full gas cost when settling this order alone on gp
    pub gas: u64,
    /// Address of the solver that provided the quote.
    pub solver: Address,
    /// Did we verify the correctness of this estimate's properties?
    pub verified: bool,
    /// Data associated with this estimation.
    #[debug(ignore)]
    pub execution: QuoteExecution,
}

impl Estimate {
    /// Returns (sell_amount, buy_amount).
    pub fn amounts(&self, query: &Query) -> (U256, U256) {
        match query.kind {
            OrderKind::Buy => (self.out_amount, query.in_amount.get()),
            OrderKind::Sell => (query.in_amount.get(), self.out_amount),
        }
    }

    /// The price of the estimate denominated in buy token.
    ///
    /// The resulting price is how many units of buy_token are bought for one
    /// unit of sell_token (buy_amount / sell_amount).
    pub fn price_in_buy_token_f64(&self, query: &Query) -> f64 {
        let (sell_amount, buy_amount) = self.amounts(query);
        f64::from(buy_amount) / f64::from(sell_amount)
    }
}

pub type PriceEstimateResult = Result<Estimate, PriceEstimationError>;

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
pub trait PriceEstimating: Send + Sync + 'static {
    fn estimate(&self, query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult>;
}

pub const HEALTHY_PRICE_ESTIMATION_TIME: Duration = Duration::from_millis(5_000);

pub async fn rate_limited<T>(
    rate_limiter: Arc<RateLimiter>,
    estimation: impl Future<Output = Result<T, PriceEstimationError>>,
) -> Result<T, PriceEstimationError> {
    let timed_estimation = async move {
        let start = Instant::now();
        let result = estimation.await;
        (start.elapsed(), result)
    };
    let rate_limited_estimation =
        rate_limiter.execute(timed_estimation, |(estimation_time, result)| {
            let too_slow = *estimation_time > HEALTHY_PRICE_ESTIMATION_TIME;
            let api_rate_limited = matches!(result, Err(PriceEstimationError::RateLimited));
            too_slow || api_rate_limited
        });
    match rate_limited_estimation.await {
        Ok((_estimation_time, Ok(result))) => Ok(result),
        // return original PriceEstimationError
        Ok((_estimation_time, Err(err))) => Err(err),
        // convert the RateLimiterError to a PriceEstimationError
        Err(_) => Err(PriceEstimationError::RateLimited),
    }
}

pub mod mocks {
    use {super::*, anyhow::anyhow, futures::FutureExt};

    pub struct FakePriceEstimator(pub Estimate);
    impl PriceEstimating for FakePriceEstimator {
        fn estimate(&self, _query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult> {
            async { Ok(self.0.clone()) }.boxed()
        }
    }

    pub struct FailingPriceEstimator;
    impl PriceEstimating for FailingPriceEstimator {
        fn estimate(&self, _query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult> {
            async {
                Err(PriceEstimationError::EstimatorInternal(anyhow!(
                    "always fail"
                )))
            }
            .boxed()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_deserialize_estimators_empty() {
        #[derive(Deserialize)]
        struct Helper {
            _estimators: NativePriceEstimators,
        }

        assert!(toml::from_str::<Helper>("estimators = []").is_err());
        assert!(toml::from_str::<Helper>("estimators = [[]]").is_err());
    }

    #[test]
    fn toml_deserialize_estimators_single_stage() {
        let toml = r#"
        estimators = [[{type = "CoinGecko"}, {type = "OneInchSpotPriceApi"}]]
        "#;

        #[derive(Deserialize)]
        struct Helper {
            estimators: NativePriceEstimators,
        }

        let parsed: Helper = toml::from_str(toml).unwrap();
        assert_eq!(
            parsed.estimators.as_slice(),
            vec![vec![
                NativePriceEstimator::CoinGecko,
                NativePriceEstimator::OneInchSpotPriceApi,
            ]]
        );
    }

    #[test]
    fn toml_deserialize_estimators_multiple_stages() {
        let toml = r#"
        estimators = [
            [{type = "CoinGecko"}, {type = "Driver", name = "solver1", url = "http://localhost:8080"}],
            [{type = "Forwarder", url = "http://localhost:12088"}],
        ]
        "#;

        #[derive(Deserialize)]
        struct Helper {
            estimators: NativePriceEstimators,
        }

        let parsed: Helper = toml::from_str(toml).unwrap();
        assert_eq!(
            parsed.estimators.as_slice(),
            vec![
                vec![
                    NativePriceEstimator::CoinGecko,
                    NativePriceEstimator::Driver(ExternalSolver {
                        name: "solver1".to_string(),
                        url: "http://localhost:8080".parse().unwrap(),
                    }),
                ],
                vec![NativePriceEstimator::Forwarder {
                    url: "http://localhost:12088".parse().unwrap(),
                }],
            ]
        );
    }

    #[test]
    fn toml_deserialize_estimators_default() {
        let estimators = NativePriceEstimators::default();
        assert!(estimators.as_slice().is_empty());
    }
}
