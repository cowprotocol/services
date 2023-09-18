use {
    crate::{
        arguments::{display_option, CodeSimulatorKind},
        conversions::U256Ext,
        rate_limiter::{RateLimiter, RateLimitingStrategy},
        trade_finding::Interaction,
    },
    anyhow::{Context, Result},
    ethcontract::{H160, U256},
    futures::future::BoxFuture,
    model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    num::BigRational,
    number::nonzero::U256 as NonZeroU256,
    reqwest::Url,
    serde::{Deserialize, Serialize},
    std::{
        cmp::{Eq, PartialEq},
        fmt::{self, Display, Formatter},
        future::Future,
        hash::Hash,
        str::FromStr,
        sync::Arc,
        time::{Duration, Instant},
    },
    thiserror::Error,
};

pub mod balancer_sor;
pub mod baseline;
pub mod competition;
pub mod external;
pub mod factory;
pub mod gas;
pub mod http;
pub mod instrumented;
pub mod native;
pub mod native_price_cache;
pub mod oneinch;
pub mod paraswap;
pub mod sanitized;
pub mod trade_finder;
pub mod zeroex;

#[derive(Clone, Debug)]
pub struct PriceEstimators(Vec<PriceEstimator>);

impl PriceEstimators {
    fn none() -> Self {
        Self(Vec::new())
    }

    pub fn as_slice(&self) -> &[PriceEstimator] {
        &self.0
    }
}

impl Default for PriceEstimators {
    fn default() -> Self {
        Self(vec![PriceEstimator {
            kind: PriceEstimatorKind::Baseline,
            address: H160::zero(),
        }])
    }
}

impl Display for PriceEstimators {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mut it = self.as_slice().iter();
        if let Some(PriceEstimator { kind, address }) = it.next() {
            write!(f, "{kind:?}|{address:?}")?;
            for PriceEstimator { kind, address } in it {
                write!(f, ",{kind:?}|{address:?}")?;
            }
            Ok(())
        } else {
            f.write_str("None")
        }
    }
}

impl FromStr for PriceEstimators {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "None" {
            return Ok(Self::none());
        }

        Ok(Self(
            s.split(',')
                .map(PriceEstimator::from_str)
                .collect::<Result<_>>()?,
        ))
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub enum PriceEstimatorKind {
    Baseline,
    Paraswap,
    ZeroEx,
    OneInch,
    BalancerSor,
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct PriceEstimator {
    pub kind: PriceEstimatorKind,
    pub address: H160,
}

impl PriceEstimator {
    /// Returns the name of this price estimator type.
    pub fn name(&self) -> String {
        format!("{:?}", self.kind)
    }
}

impl FromStr for PriceEstimator {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (estimator, address) = s
            .split_once('|')
            .unwrap_or((s, "0x0000000000000000000000000000000000000000"));
        let address = H160::from_str(address).context("failed to convert to H160: {address}")?;
        let kind = match estimator {
            "Baseline" => PriceEstimatorKind::Baseline,
            "Paraswap" => PriceEstimatorKind::Paraswap,
            "ZeroEx" => PriceEstimatorKind::ZeroEx,
            "OneInch" => PriceEstimatorKind::OneInch,
            "BalancerSor" => PriceEstimatorKind::BalancerSor,
            estimator => {
                anyhow::bail!("failed to convert to PriceEstimatorKind: {estimator}")
            }
        };
        Ok(PriceEstimator { kind, address })
    }
}

/// Shared price estimation configuration arguments.
#[derive(clap::Parser)]
#[group(skip)]
pub struct Arguments {
    /// Configures the back off strategy for price estimators when requests take
    /// too long. Requests issued while back off is active get dropped
    /// entirely. Needs to be passed as
    /// "<back_off_growth_factor>,<min_back_off>,<max_back_off>".
    /// back_off_growth_factor: f64 >= 1.0
    /// min_back_off: f64 in seconds
    /// max_back_off: f64 in seconds
    #[clap(long, env, verbatim_doc_comment)]
    pub price_estimation_rate_limiter: Option<RateLimitingStrategy>,

    /// How often the native price estimator should refresh its cache.
    #[clap(
        long,
        env,
        default_value = "1",
        value_parser = crate::arguments::duration_from_seconds,
    )]
    pub native_price_cache_refresh_secs: Duration,

    /// How long cached native prices stay valid.
    #[clap(
        long,
        env,
        default_value = "30",
        value_parser = crate::arguments::duration_from_seconds,
    )]
    pub native_price_cache_max_age_secs: Duration,

    /// How long before expiry the native price cache should try to update the
    /// price in the background. This is useful to make sure that prices are
    /// usable at all times. This value has to be smaller than
    /// `--native-price-cache-max-age-secs`.
    #[clap(
        long,
        env,
        default_value = "2",
        value_parser = crate::arguments::duration_from_seconds,
    )]
    pub native_price_prefetch_time_secs: Duration,

    /// How many cached native token prices can be updated at most in one
    /// maintenance cycle.
    #[clap(long, env, default_value = "3")]
    pub native_price_cache_max_update_size: usize,

    /// How many price estimation requests can be executed concurrently in the
    /// maintenance task.
    #[clap(long, env, default_value = "1")]
    pub native_price_cache_concurrent_requests: usize,

    /// The amount in native tokens atoms to use for price estimation. Should be
    /// reasonably large so that small pools do not influence the prices. If
    /// not set a reasonable default is used based on network id.
    #[clap(long, env, value_parser = U256::from_dec_str)]
    pub amount_to_estimate_prices_with: Option<U256>,

    /// The API endpoint to call the Quasimodo solver for price estimation
    #[clap(long, env)]
    pub quasimodo_solver_url: Option<Url>,

    /// The API endpoint to call the yearn solver for price estimation
    #[clap(long, env)]
    pub yearn_solver_url: Option<Url>,

    /// The API path to use for solving.
    #[clap(long, env, default_value = "solve")]
    pub yearn_solver_path: String,

    /// The API endpoint to call the Raven solver for price estimation
    #[clap(long, env)]
    pub raven_solver_url: Option<Url>,

    /// The API path to use for solving.
    #[clap(long, env, default_value = "solve")]
    pub raven_solver_path: String,

    /// The API endpoint for the Balancer SOR API for solving.
    #[clap(long, env)]
    pub balancer_sor_url: Option<Url>,

    /// The trade simulation strategy to use for supported price estimators.
    /// This ensures that the proposed trade calldata gets simulated, thus
    /// avoiding invalid calldata mistakenly advertising unachievable prices
    /// when quoting, as well as more robustly identifying unsupported
    /// tokens. The `Web3` simulator requires the `--simulation-node_url`
    /// parameter to be set. The `Tenderly` simulator requires `--tenderly-*`
    /// parameters to be set.
    #[clap(long, env)]
    pub trade_simulator: Option<CodeSimulatorKind>,

    /// Flag to enable saving Tenderly simulations in the dashboard for
    /// successful trade simulations.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub tenderly_save_successful_trade_simulations: bool,

    /// Flag to enable saving Tenderly simulations in the dashboard for failed
    /// trade simulations. This helps debugging reverted quote simulations.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub tenderly_save_failed_trade_simulations: bool,

    /// Use 0x estimator for only buy orders. This flag can be enabled to reduce
    /// request pressure on the 0x API.
    #[clap(long, env, action = clap::ArgAction::Set, default_value = "false")]
    pub zeroex_only_estimate_buy_queries: bool,
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        display_option(
            f,
            "price_estimation_rate_limites",
            &self.price_estimation_rate_limiter,
        )?;
        writeln!(
            f,
            "native_price_cache_refresh_secs: {:?}",
            self.native_price_cache_refresh_secs
        )?;
        writeln!(
            f,
            "native_price_cache_max_age_secs: {:?}",
            self.native_price_cache_max_age_secs
        )?;
        writeln!(
            f,
            "native_price_prefetch_time_secs: {:?}",
            self.native_price_prefetch_time_secs
        )?;
        writeln!(
            f,
            "native_price_cache_max_update_size: {}",
            self.native_price_cache_max_update_size
        )?;
        writeln!(
            f,
            "native_price_cache_concurrent_requests: {}",
            self.native_price_cache_concurrent_requests
        )?;
        display_option(
            f,
            "amount_to_estimate_prices_with",
            &self.amount_to_estimate_prices_with,
        )?;
        display_option(f, "quasimodo_solver_url", &self.quasimodo_solver_url)?;
        display_option(f, "yearn_solver_url", &self.yearn_solver_url)?;
        writeln!(f, "yearn_solver_path: {}", self.yearn_solver_path)?;
        display_option(f, "raven_solver_url", &self.raven_solver_url)?;
        writeln!(f, "raven_solver_path: {}", self.raven_solver_path)?;
        display_option(f, "balancer_sor_url", &self.balancer_sor_url)?;
        display_option(
            f,
            "trade_simulator",
            &self
                .trade_simulator
                .as_ref()
                .map(|value| format!("{value:?}")),
        )?;
        writeln!(
            f,
            "tenderly_save_successful_trade_simulations: {}",
            self.tenderly_save_successful_trade_simulations
        )?;
        writeln!(
            f,
            "tenderly_save_failed_trade_simulations: {}",
            self.tenderly_save_failed_trade_simulations
        )?;
        writeln!(
            f,
            "zeroex_only_estimate_buy_queries: {:?}",
            self.zeroex_only_estimate_buy_queries
        )?;

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum PriceEstimationError {
    #[error("token {token:?} is not supported: {reason:}")]
    UnsupportedToken { token: H160, reason: String },

    #[error("No liquidity")]
    NoLiquidity,

    #[error("Unsupported Order Type")]
    UnsupportedOrderType(String),

    #[error("Rate limited")]
    RateLimited,

    #[error(transparent)]
    EstimatorInternal(anyhow::Error),

    #[error(transparent)]
    ProtocolInternal(anyhow::Error),
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
            Self::EstimatorInternal(err) => Self::EstimatorInternal(crate::clone_anyhow_error(err)),
            Self::ProtocolInternal(err) => Self::ProtocolInternal(crate::clone_anyhow_error(err)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Query {
    pub sell_token: H160,
    pub buy_token: H160,
    /// For OrderKind::Sell amount is in sell_token and for OrderKind::Buy in
    /// buy_token.
    pub in_amount: NonZeroU256,
    pub kind: OrderKind,
    /// If this is `Some` the quotes are expected to pass simulations using the
    /// contained parameters.
    pub verification: Option<Verification>,
}

/// Conditions under which a given price estimate needs to work in order to be
/// viable.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Verification {
    /// This address needs to have the `sell_token`.
    pub from: H160,
    /// This address will receive the `buy_token`.
    pub receiver: H160,
    /// These interactions will be executed before the trade.
    pub pre_interactions: Vec<Interaction>,
    /// These interactions will be executed after the trade.
    pub post_interactions: Vec<Interaction>,
    /// `sell_token` will be taken via this approach.
    pub sell_token_source: SellTokenSource,
    /// `buy_token` will be sent via this approach.
    pub buy_token_destination: BuyTokenDestination,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize)]
pub struct Estimate {
    pub out_amount: U256,
    /// full gas cost when settling this order alone on gp
    pub gas: u64,
    /// Address of the solver that provided the quote.
    pub solver: H160,
}

impl Estimate {
    /// Returns (sell_amount, buy_amount).
    pub fn amounts(&self, query: &Query) -> (U256, U256) {
        match query.kind {
            OrderKind::Buy => (self.out_amount, query.in_amount.get()),
            OrderKind::Sell => (query.in_amount.get(), self.out_amount),
        }
    }

    /// The resulting price is how many units of sell_token needs to be sold for
    /// one unit of buy_token (sell_amount / buy_amount).
    pub fn price_in_sell_token_rational(&self, query: &Query) -> Option<BigRational> {
        let (sell_amount, buy_amount) = self.amounts(query);
        amounts_to_price(sell_amount, buy_amount)
    }

    /// The price for the estimate denominated in sell token.
    ///
    /// The resulting price is how many units of sell_token needs to be sold for
    /// one unit of buy_token (sell_amount / buy_amount).
    pub fn price_in_sell_token_f64(&self, query: &Query) -> f64 {
        let (sell_amount, buy_amount) = self.amounts(query);
        sell_amount.to_f64_lossy() / buy_amount.to_f64_lossy()
    }

    /// The price of the estimate denominated in buy token.
    ///
    /// The resulting price is how many units of buy_token are bought for one
    /// unit of sell_token (buy_amount / sell_amount).
    pub fn price_in_buy_token_f64(&self, query: &Query) -> f64 {
        let (sell_amount, buy_amount) = self.amounts(query);
        buy_amount.to_f64_lossy() / sell_amount.to_f64_lossy()
    }
}

pub type PriceEstimateResult = Result<Estimate, PriceEstimationError>;

#[mockall::automock]
pub trait PriceEstimating: Send + Sync + 'static {
    fn estimate(&self, query: Arc<Query>) -> BoxFuture<'_, PriceEstimateResult>;
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
            async { Ok(self.0) }.boxed()
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
    fn parse_price_estimators() {
        for arg in [
            "Baselin|0x0000000000000000000000000000000000000001", // incorrect estimator name
            "Baseline|0x000000000000000000000000000000000000000", // address too short
            "Baseline|0x00000000000000000000000000000000000000010", // address too long
            "Baseline,0x0000000000000000000000000000000000000001", // wrong separator
            "Baseline|",                                          // missing argument
            "Baseline|0x0000000000000000000000000000000000000001|", // additional argument
            "Baseline|0x0000000000000000000000000000000000000001|Baseline", // additional argument
        ] {
            let parsed = arg.parse::<PriceEstimator>();
            assert!(
                parsed.is_err(),
                "string successfully parsed when it should have failed: {arg}"
            );
        }

        let address = H160::from_low_u64_be;
        let parsed = |arg: &str| arg.parse::<PriceEstimator>().unwrap();
        let estimator = |kind, address| PriceEstimator { kind, address };
        // Fallback case to allow for default CLI arguments.
        assert_eq!(
            parsed("Baseline"),
            estimator(PriceEstimatorKind::Baseline, address(0))
        );
        assert_eq!(
            parsed("Baseline|0x0000000000000000000000000000000000000001"),
            estimator(PriceEstimatorKind::Baseline, address(1))
        );
        assert_eq!(
            parsed("ZeroEx|0x0000000000000000000000000000000000000001"),
            estimator(PriceEstimatorKind::ZeroEx, address(1))
        );
        assert_eq!(
            parsed("OneInch|0x0000000000000000000000000000000000000001"),
            estimator(PriceEstimatorKind::OneInch, address(1))
        );
        assert_eq!(
            parsed("BalancerSor|0x0000000000000000000000000000000000000001"),
            estimator(PriceEstimatorKind::BalancerSor, address(1))
        );
    }
}
