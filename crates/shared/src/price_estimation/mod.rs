use {
    self::trade_verifier::balance_overrides,
    crate::{
        arguments::{self, display_option, display_secret_option},
        trade_finding::{Interaction, QuoteExecution},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result, ensure},
    bigdecimal::BigDecimal,
    futures::future::BoxFuture,
    itertools::Itertools,
    model::order::{BuyTokenDestination, OrderKind, SellTokenSource},
    number::nonzero::NonZeroU256,
    rate_limit::{RateLimiter, Strategy},
    reqwest::Url,
    serde::{Deserialize, Serialize},
    std::{
        cmp::{Eq, PartialEq},
        error::Error,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalSolver {
    pub name: String,
    pub url: Url,
}

impl From<arguments::ExternalSolver> for ExternalSolver {
    fn from(value: arguments::ExternalSolver) -> Self {
        Self {
            name: value.name,
            url: value.url,
        }
    }
}

impl FromStr for ExternalSolver {
    type Err = anyhow::Error;

    fn from_str(solver: &str) -> Result<Self> {
        let parts: Vec<&str> = solver.split('|').collect();
        ensure!(parts.len() >= 2, "not enough arguments for external solver");
        let (name, url) = (parts[0], parts[1]);
        let url: Url = url.parse()?;
        Ok(Self {
            name: name.to_owned(),
            url,
        })
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum NativePriceEstimator {
    Driver(ExternalSolver),
    Forwarder(Url),
    OneInchSpotPriceApi,
    CoinGecko,
}

impl Display for NativePriceEstimator {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let formatter = match self {
            NativePriceEstimator::Driver(s) => format!("Driver|{}|{}", &s.name, s.url),
            NativePriceEstimator::Forwarder(url) => format!("Forwarder|{}", url),
            NativePriceEstimator::OneInchSpotPriceApi => "OneInchSpotPriceApi".into(),
            NativePriceEstimator::CoinGecko => "CoinGecko".into(),
        };
        write!(f, "{formatter}")
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
        write!(f, "{formatter}")
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
        let (variant, args) = s.split_once('|').unwrap_or((s, ""));
        match variant {
            "OneInchSpotPriceApi" => Ok(NativePriceEstimator::OneInchSpotPriceApi),
            "CoinGecko" => Ok(NativePriceEstimator::CoinGecko),
            "Driver" => Ok(NativePriceEstimator::Driver(ExternalSolver::from_str(
                args,
            )?)),
            "Forwarder" => Ok(NativePriceEstimator::Forwarder(
                args.parse()
                    .context("Forwarder price estimator invalid URL")?,
            )),
            _ => Err(anyhow::anyhow!("unsupported native price estimator: {}", s)),
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
    /// price in the background. This value has to be smaller than
    /// `--native-price-cache-max-age`.
    #[clap(
        long,
        env,
        default_value = "80s",
        value_parser = humantime::parse_duration,
    )]
    pub native_price_prefetch_time: Duration,

    /// How many price estimation requests can be executed concurrently in the
    /// maintenance task.
    #[clap(long, env, default_value = "1")]
    pub native_price_cache_concurrent_requests: usize,

    /// The amount in native tokens atoms to use for price estimation. Should be
    /// reasonably large so that small pools do not influence the prices. If
    /// not set a reasonable default is used based on network id.
    #[clap(long, env)]
    pub amount_to_estimate_prices_with: Option<alloy::primitives::U256>,

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

    /// Default timeout for quote requests.
    #[clap(
        long,
        env,
        default_value = "5s",
        value_parser = humantime::parse_duration,
    )]
    pub quote_timeout: Duration,

    #[clap(flatten)]
    pub balance_overrides: balance_overrides::Arguments,

    /// List of mappings of native price tokens substitutions with approximated
    /// value from other token:
    /// "<token1>|<approx_token1>,<token2>|<approx_token2>"
    /// - token1 is a token address for which we get the native token price
    /// - approx_token1 is a token address used for the price approximation
    #[clap(
        long,
        env,
        value_delimiter = ',',
        value_parser = parse_tuple::<Address, Address>
    )]
    pub native_price_approximation_tokens: Vec<(Address, Address)>,

    /// Tokens for which quote verification should not be attempted. This is an
    /// escape hatch when there is a very bad but verifiable liquidity source
    /// that would win against a very good but unverifiable liquidity source
    /// (e.g. private liquidity that exists but can't be verified).
    #[clap(long, env, value_delimiter = ',')]
    pub tokens_without_verification: Vec<Address>,
}

/// Custom Clap parser for tuple pair
fn parse_tuple<T, U>(input: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = input.find('|').ok_or_else(|| {
        format!(
            "invalid pair values delimiter character, expected: 'value1|value2', got: '{input}'"
        )
    })?;
    Ok((
        input[..pos].trim().parse()?,
        input[pos + 1..].trim().parse()?,
    ))
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
            native_price_cache_concurrent_requests,
            amount_to_estimate_prices_with,
            balancer_sor_url,
            one_inch_api_key,
            one_inch_url,
            coin_gecko,
            quote_inaccuracy_limit,
            quote_verification,
            quote_timeout,
            balance_overrides,
            native_price_approximation_tokens,
            tokens_without_verification,
        } = self;

        display_option(
            f,
            "price_estimation_rate_limites",
            price_estimation_rate_limiter,
        )?;
        writeln!(
            f,
            "native_price_cache_refresh: {native_price_cache_refresh:?}"
        )?;
        writeln!(
            f,
            "native_price_cache_max_age: {native_price_cache_max_age:?}"
        )?;
        writeln!(
            f,
            "native_price_prefetch_time: {native_price_prefetch_time:?}"
        )?;
        writeln!(
            f,
            "native_price_cache_concurrent_requests: {native_price_cache_concurrent_requests}"
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
        writeln!(f, "one_inch_spot_price_api_url: {one_inch_url}")?;
        display_secret_option(
            f,
            "coin_gecko_api_key: {:?}",
            coin_gecko.coin_gecko_api_key.as_ref(),
        )?;
        writeln!(f, "coin_gecko_api_url: {}", coin_gecko.coin_gecko_url)?;
        writeln!(f, "coin_gecko_api_url: {}", coin_gecko.coin_gecko_url)?;
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
        writeln!(f, "quote_inaccuracy_limit: {quote_inaccuracy_limit}")?;
        writeln!(f, "quote_verification: {quote_verification:?}")?;
        writeln!(f, "quote_timeout: {quote_timeout:?}")?;
        write!(f, "{balance_overrides}")?;
        writeln!(
            f,
            "native_price_approximation_tokens: {native_price_approximation_tokens:?}"
        )?;
        writeln!(
            f,
            "tokens_without_verification: {tokens_without_verification:?}"
        )?;

        Ok(())
    }
}

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
    /// These interactions will be executed before the trade.
    pub pre_interactions: Vec<Interaction>,
    /// These interactions will be executed after the trade.
    pub post_interactions: Vec<Interaction>,
    /// `sell_token` will be taken via this approach.
    pub sell_token_source: SellTokenSource,
    /// `buy_token` will be sent via this approach.
    pub buy_token_destination: BuyTokenDestination,
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
    use {super::*, alloy::primitives::address, clap::Parser};

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
                ExternalSolver::from_str(
                    "baseline|http://localhost:1234/|0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
                )
                .unwrap(),
            )
            .to_string(),
            &NativePriceEstimator::OneInchSpotPriceApi.to_string(),
            &NativePriceEstimator::Forwarder("http://localhost:9588".parse().unwrap()).to_string(),
            "Driver|one|http://localhost:1111/,Driver|two|http://localhost:2222/;Driver|three|http://localhost:3333/,Driver|four|http://localhost:4444/",
            &format!(
                "Driver|one|http://localhost:1111/,Driver|two|http://localhost:2222/;{},Driver|four|http://localhost:4444/",
                NativePriceEstimator::OneInchSpotPriceApi
            ),
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

    #[test]
    fn test_parse_tuple() {
        let result = parse_tuple::<Address, Address>(
            "0102030405060708091011121314151617181920|a1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0",
        )
        .unwrap();
        assert_eq!(
            result.0,
            address!("0102030405060708091011121314151617181920")
        );
        assert_eq!(
            result.1,
            address!("a1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0")
        );

        let result = parse_tuple::<Address, Address>(
            "0102030405060708091011121314151617181920 a1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0",
        );
        assert!(result.is_err());

        // test parsing with delimiter
        #[derive(Parser)]
        struct Cli {
            #[arg(value_delimiter = ',', value_parser = parse_tuple::<Address, Address>)]
            param: Vec<(Address, Address)>,
        }
        let cli = Cli::parse_from(vec![
            "",
            r#"0102030405060708091011121314151617181920|a1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0,
            f102030405060708091011121314151617181920|f1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0"#,
        ]);

        assert_eq!(
            cli.param[0],
            (
                address!("0102030405060708091011121314151617181920"),
                address!("a1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0")
            )
        );
        assert_eq!(
            cli.param[1],
            (
                address!("f102030405060708091011121314151617181920"),
                address!("f1a2a3a4a5a6a7a8a9a0a1a2a3a4a5a6a7a8a9a0")
            )
        );
    }
}
