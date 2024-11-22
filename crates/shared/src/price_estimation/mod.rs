use {
    self::trade_verifier::balance_overrides::ConfigurationBalanceOverrides,
    crate::{
        arguments::{display_option, display_secret_option, ExternalSolver},
        trade_finding::Interaction,
    },
    anyhow::Result,
    bigdecimal::BigDecimal,
    ethcontract::{H160, U256},
    futures::future::BoxFuture,
    itertools::Itertools,
    model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    number::nonzero::U256 as NonZeroU256,
    rate_limit::{RateLimiter, Strategy},
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

mod buffered;
pub mod competition;
pub mod external;
pub mod factory;
pub mod gas;
pub mod instrumented;
pub mod native;
pub mod native_price_cache;
pub mod sanitized;
pub mod trade_finder;
pub mod trade_verifier;

#[derive(Clone, Debug)]
pub struct NativePriceEstimators(Vec<Vec<NativePriceEstimator>>);

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum NativePriceEstimator {
    Driver(ExternalSolver),
    OneInchSpotPriceApi,
    CoinGecko,
}

impl Display for NativePriceEstimator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let formatter = match self {
            NativePriceEstimator::Driver(s) => format!("{}|{}", &s.name, s.url),
            NativePriceEstimator::OneInchSpotPriceApi => "OneInchSpotPriceApi".into(),
            NativePriceEstimator::CoinGecko => "CoinGecko".into(),
        };
        write!(f, "{}", formatter)
    }
}

impl NativePriceEstimators {
    pub fn as_slice(&self) -> &[Vec<NativePriceEstimator>] {
        &self.0
    }
}

impl Display for NativePriceEstimators {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let formatter = self
            .as_slice()
            .iter()
            .map(|stage| {
                stage
                    .iter()
                    .format_with(",", |estimator, f| f(&format_args!("{estimator}")))
            })
            .format(";");
        write!(f, "{}", formatter)
    }
}

impl FromStr for NativePriceEstimators {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            s.split(';')
                .map(|sub_list| {
                    sub_list
                        .split(',')
                        .map(NativePriceEstimator::from_str)
                        .collect::<Result<Vec<NativePriceEstimator>>>()
                })
                .collect::<Result<Vec<Vec<NativePriceEstimator>>>>()?,
        ))
    }
}

impl FromStr for NativePriceEstimator {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "OneInchSpotPriceApi" => Ok(NativePriceEstimator::OneInchSpotPriceApi),
            "CoinGecko" => Ok(NativePriceEstimator::CoinGecko),
            estimator => Ok(NativePriceEstimator::Driver(ExternalSolver::from_str(
                estimator,
            )?)),
        }
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
    /// min_back_off: Duration
    /// max_back_off: Duration
    #[clap(long, env, verbatim_doc_comment)]
    pub price_estimation_rate_limiter: Option<Strategy>,

    /// How often the native price estimator should refresh its cache.
    #[clap(
        long,
        env,
        default_value = "1s",
        value_parser = humantime::parse_duration,
    )]
    pub native_price_cache_refresh: Duration,

    /// How long cached native prices stay valid.
    #[clap(
        long,
        env,
        default_value = "10m",
        value_parser = humantime::parse_duration,
    )]
    pub native_price_cache_max_age: Duration,

    /// How long before expiry the native price cache should try to update the
    /// price in the background. This is useful to make sure that prices are
    /// usable at all times. This value has to be smaller than
    /// `--native-price-cache-max-age`.
    #[clap(
        long,
        env,
        default_value = "80s",
        value_parser = humantime::parse_duration,
    )]
    pub native_price_prefetch_time: Duration,

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

    /// The API endpoint for the Balancer SOR API for solving.
    #[clap(long, env)]
    pub balancer_sor_url: Option<Url>,

    /// The API key for the 1Inch API.
    #[clap(long, env)]
    pub one_inch_api_key: Option<String>,

    /// The base URL for the 1Inch API.
    #[clap(long, env, default_value = "https://api.1inch.dev/")]
    pub one_inch_url: Url,

    /// The CoinGecko native price configuration
    #[clap(flatten)]
    pub coin_gecko: CoinGecko,

    /// How inaccurate a quote must be before it gets discarded provided as a
    /// factor.
    /// E.g. a value of `0.01` means at most 1 percent of the sell or buy tokens
    /// can be paid out of the settlement contract buffers.
    #[clap(long, env, default_value = "1.")]
    pub quote_inaccuracy_limit: BigDecimal,

    /// How strict quote verification should be.
    #[clap(
        long,
        env,
        default_value = "unverified",
        value_enum,
        verbatim_doc_comment
    )]
    pub quote_verification: QuoteVerificationMode,

    /// Time solvers have to compute a quote
    #[clap(
        long,
        env,
        default_value = "5s",
        value_parser = humantime::parse_duration,
    )]
    pub quote_timeout: Duration,

    /// Token configuration for simulated balances on verified quotes.
    #[clap(long, env, default_value_t)]
    pub quote_token_balance_overrides: ConfigurationBalanceOverrides,
}

#[derive(clap::Parser)]
pub struct CoinGecko {
    /// The API key for the CoinGecko API.
    #[clap(long, env)]
    pub coin_gecko_api_key: Option<String>,

    /// The base URL for the CoinGecko API.
    #[clap(
        long,
        env,
        default_value = "https://api.coingecko.com/api/v3/simple/token_price"
    )]
    pub coin_gecko_url: Url,

    #[clap(flatten)]
    pub coin_gecko_buffered: Option<CoinGeckoBuffered>,
}

#[derive(clap::Parser)]
#[clap(group(
    clap::ArgGroup::new("coin_gecko_buffered")
    .requires_all(&[
        "coin_gecko_debouncing_time",
        "coin_gecko_result_ready_timeout",
        "coin_gecko_broadcast_channel_capacity"
    ])
    .multiple(true)
    .required(false),
))]
pub struct CoinGeckoBuffered {
    /// An additional minimum delay to wait for collecting CoinGecko requests.
    ///
    /// The delay to start counting after receiving the first request.
    #[clap(long, env, value_parser = humantime::parse_duration, group = "coin_gecko_buffered")]
    pub coin_gecko_debouncing_time: Option<Duration>,

    /// The timeout to wait for the result to be ready
    #[clap(long, env, value_parser = humantime::parse_duration, group = "coin_gecko_buffered")]
    pub coin_gecko_result_ready_timeout: Option<Duration>,

    /// Maximum capacity of the broadcast channel to store the CoinGecko native
    /// prices results
    #[clap(long, env, group = "coin_gecko_buffered")]
    pub coin_gecko_broadcast_channel_capacity: Option<usize>,
}

/// Controls which level of quote verification gets applied.
#[derive(Copy, Clone, Debug, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
pub enum QuoteVerificationMode {
    /// Quotes do not get verified.
    Unverified,
    /// Quotes get verified whenever possible and verified
    /// quotes are preferred over unverified ones.
    Prefer,
    /// Quotes get discarded if they can't be verified.
    /// Some scenarios like missing sell token balance are exempt.
    EnforceWhenPossible,
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let Self {
            price_estimation_rate_limiter,
            native_price_cache_refresh,
            native_price_cache_max_age,
            native_price_prefetch_time,
            native_price_cache_max_update_size,
            native_price_cache_concurrent_requests,
            amount_to_estimate_prices_with,
            balancer_sor_url,
            one_inch_api_key,
            one_inch_url,
            coin_gecko,
            quote_inaccuracy_limit,
            quote_verification,
            quote_timeout,
            quote_token_balance_overrides,
        } = self;

        display_option(
            f,
            "price_estimation_rate_limites",
            price_estimation_rate_limiter,
        )?;
        writeln!(
            f,
            "native_price_cache_refresh: {:?}",
            native_price_cache_refresh
        )?;
        writeln!(
            f,
            "native_price_cache_max_age: {:?}",
            native_price_cache_max_age
        )?;
        writeln!(
            f,
            "native_price_prefetch_time: {:?}",
            native_price_prefetch_time
        )?;
        writeln!(
            f,
            "native_price_cache_max_update_size: {}",
            native_price_cache_max_update_size
        )?;
        writeln!(
            f,
            "native_price_cache_concurrent_requests: {}",
            native_price_cache_concurrent_requests
        )?;
        display_option(
            f,
            "amount_to_estimate_prices_with: {}",
            amount_to_estimate_prices_with,
        )?;
        display_option(f, "balancer_sor_url", balancer_sor_url)?;
        display_secret_option(
            f,
            "one_inch_spot_price_api_key: {:?}",
            one_inch_api_key.as_ref(),
        )?;
        writeln!(f, "one_inch_spot_price_api_url: {}", one_inch_url)?;
        display_secret_option(
            f,
            "coin_gecko_api_key: {:?}",
            coin_gecko.coin_gecko_api_key.as_ref(),
        )?;
        writeln!(f, "coin_gecko_api_url: {}", coin_gecko.coin_gecko_url)?;
        writeln!(f, "coin_gecko_api_url: {}", coin_gecko.coin_gecko_url)?;
        writeln!(
            f,
            "coin_gecko_result_ready_timeout: {:?}",
            coin_gecko
                .coin_gecko_buffered
                .as_ref()
                .map(|coin_gecko_buffered| coin_gecko_buffered.coin_gecko_result_ready_timeout),
        )?;
        writeln!(
            f,
            "coin_gecko_debouncing_time: {:?}",
            coin_gecko
                .coin_gecko_buffered
                .as_ref()
                .map(|coin_gecko_buffered| coin_gecko_buffered.coin_gecko_debouncing_time),
        )?;
        writeln!(
            f,
            "coin_gecko_broadcast_channel_capacity: {:?}",
            coin_gecko.coin_gecko_buffered.as_ref().map(
                |coin_gecko_buffered| coin_gecko_buffered.coin_gecko_broadcast_channel_capacity
            ),
        )?;
        writeln!(f, "quote_inaccuracy_limit: {}", quote_inaccuracy_limit)?;
        writeln!(f, "quote_verification: {:?}", quote_verification)?;
        writeln!(f, "quote_timeout: {:?}", quote_timeout)?;
        writeln!(
            f,
            "quote_token_balance_overrides: {:?}",
            quote_token_balance_overrides
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
    pub verification: Verification,
    /// Signals whether responses from that were valid on previous blocks can be
    /// used to answer the query.
    #[serde(skip_serializing)]
    pub block_dependent: bool,
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
    /// Did we verify the correctness of this estimate's properties?
    pub verified: bool,
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
        buy_amount.to_f64_lossy() / sell_amount.to_f64_lossy()
    }
}

pub type PriceEstimateResult = Result<Estimate, PriceEstimationError>;

#[mockall::automock]
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
    use {super::*, clap::Parser};

    #[test]
    fn string_repr_round_trip_native_price_estimators() {
        // We use NativePriceEstimators as one of the types used in an Arguments object
        // that derives clap::Parser. Clap parsing of an argument using
        // default_value_t requires that std::fmt::Display roundtrips correctly with the
        // Arg::value_parser or #[arg(value_enum)]:
        // https://docs.rs/clap/latest/clap/_derive/index.html#arg-attributes

        let parsed = |arg: &str| NativePriceEstimators::from_str(arg);
        let stringified = |arg: &NativePriceEstimators| format!("{arg}");

        for repr in [
            &NativePriceEstimator::Driver(
                ExternalSolver::from_str("baseline|http://localhost:1234/").unwrap(),
            )
            .to_string(),
            &NativePriceEstimator::OneInchSpotPriceApi.to_string(),
            "one|http://localhost:1111/,two|http://localhost:2222/;three|http://localhost:3333/,four|http://localhost:4444/",
            &format!("one|http://localhost:1111/,two|http://localhost:2222/;{},four|http://localhost:4444/", NativePriceEstimator::OneInchSpotPriceApi),
        ] {
            assert_eq!(stringified(&parsed(repr).unwrap()), repr);
        }
    }

    #[test]
    fn enable_coin_gecko_buffered() {
        let args = vec![
            "test", // Program name
            "--coin-gecko-api-key",
            "someapikey",
            "--coin-gecko-url",
            "https://api.coingecko.com/api/v3/simple/token_price",
            "--coin-gecko-debouncing-time",
            "300ms",
            "--coin-gecko-result-ready-timeout",
            "600ms",
            "--coin-gecko-broadcast-channel-capacity",
            "50",
        ];

        let coin_gecko = CoinGecko::parse_from(args);

        assert!(coin_gecko.coin_gecko_buffered.is_some());

        let buffered = coin_gecko.coin_gecko_buffered.unwrap();
        assert_eq!(
            buffered.coin_gecko_debouncing_time.unwrap(),
            Duration::from_millis(300)
        );
        assert_eq!(
            buffered.coin_gecko_result_ready_timeout.unwrap(),
            Duration::from_millis(600)
        );
        assert_eq!(buffered.coin_gecko_broadcast_channel_capacity.unwrap(), 50);
    }

    #[test]
    fn test_without_buffered_present() {
        let args = vec![
            "test", // Program name
            "--coin-gecko-api-key",
            "someapikey",
            "--coin-gecko-url",
            "https://api.coingecko.com/api/v3/simple/token_price",
        ];

        let coin_gecko = CoinGecko::parse_from(args);

        assert!(coin_gecko.coin_gecko_buffered.is_none());
    }

    #[test]
    fn test_invalid_partial_buffered_present() {
        let args = vec![
            "test", // Program name
            "--coin-gecko-api-key",
            "someapikey",
            "--coin-gecko-url",
            "https://api.coingecko.com/api/v3/simple/token_price",
            "--coin-gecko-debouncing-time",
            "300ms",
        ];

        let result = CoinGecko::try_parse_from(args);

        assert!(result.is_err());
    }
}
