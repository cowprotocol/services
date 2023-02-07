pub mod balancer_sor;
pub mod baseline;
pub mod competition;
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

use {
    crate::{
        arguments::display_option,
        bad_token::BadTokenDetecting,
        conversions::U256Ext,
        rate_limiter::{RateLimiter, RateLimiterError, RateLimitingStrategy},
    },
    anyhow::Result,
    clap::ValueEnum,
    ethcontract::{H160, U256},
    futures::{stream::BoxStream, StreamExt},
    model::order::OrderKind,
    num::BigRational,
    reqwest::Url,
    serde::{Deserialize, Serialize},
    std::{
        cmp::{Eq, PartialEq},
        fmt::{self, Display, Formatter},
        future::Future,
        hash::Hash,
        sync::Arc,
        time::{Duration, Instant},
    },
    thiserror::Error,
};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, ValueEnum)]
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
        format!("{self:?}")
    }
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum TradeValidatorKind {
    Web3,
    Tenderly,
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

    /// The API endpoint to call the mip v2 solver for price estimation
    #[clap(long, env)]
    pub quasimodo_solver_url: Option<Url>,

    /// The API endpoint to call the yearn solver for price estimation
    #[clap(long, env)]
    pub yearn_solver_url: Option<Url>,

    /// The API endpoint for the Balancer SOR API for solving.
    #[clap(long, env)]
    pub balancer_sor_url: Option<Url>,

    /// The trade simulation strategy to use for supported price estimators.
    /// This ensures that the proposed trade calldata gets simulated, thus
    /// avoiding invalid calldata mistakenly advertising unachievable prices
    /// when quoting, as well as more robustly identifying unsupported
    /// tokens.
    #[clap(long, env)]
    pub trade_simulator: Option<TradeValidatorKind>,

    /// Flag to enable saving Tenderly simulations in the dashboard for failed
    /// trade simulations. This helps debugging reverted quote simulations.
    #[clap(long, env)]
    pub tenderly_save_failed_trade_simulations: bool,
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
            "tenderly_save_failed_trade_simulations: {}",
            self.tenderly_save_failed_trade_simulations
        )?;

        Ok(())
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Query {
    /// Optional `from` address that would be executing the query.
    pub from: Option<H160>,
    pub sell_token: H160,
    pub buy_token: H160,
    /// For OrderKind::Sell amount is in sell_token and for OrderKind::Buy in
    /// buy_token.
    pub in_amount: U256,
    pub kind: OrderKind,
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Deserialize)]
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
    // The '_ lifetime in the return value is the same as 'a but we need to write it
    // as underscore because of a mockall limitation.

    /// Returns one result for each query in arbitrary order. The usize is the
    /// index into the queries slice.
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

pub const HEALTHY_PRICE_ESTIMATION_TIME: Duration = Duration::from_millis(5_000);

pub async fn rate_limited<T, E>(
    rate_limiter: Arc<RateLimiter>,
    estimation: impl Future<Output = Result<T, E>>,
) -> Result<T, E>
where
    E: From<anyhow::Error>,
    E: From<RateLimiterError>,
{
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
        Err(err) => Err(E::from(err)),
    }
}

pub mod mocks {
    use {super::*, anyhow::anyhow};

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
