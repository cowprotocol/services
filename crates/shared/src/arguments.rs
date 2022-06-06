//! Contains command line arguments and related helpers that are shared between the binaries.
use crate::{
    gas_price_estimation::GasEstimatorType,
    http_client::RateLimitingStrategy,
    sources::{balancer_v2::BalancerFactoryKind, BaselineSource},
};
use anyhow::{ensure, Context, Result};
use ethcontract::{H160, H256, U256};
use std::{
    num::{NonZeroU64, ParseFloatError},
    str::FromStr,
    time::Duration,
};
use tracing::level_filters::LevelFilter;
use url::Url;

#[derive(Debug, clap::Parser)]
pub struct Arguments {
    #[clap(
        long,
        env,
        default_value = "warn,orderbook=debug,solver=debug,shared=debug,shared::transport::http=info"
    )]
    pub log_filter: String,

    #[clap(long, env, default_value = "error", parse(try_from_str))]
    pub log_stderr_threshold: LevelFilter,

    /// The Ethereum node URL to connect to.
    #[clap(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// Timeout in seconds for all http requests.
    #[clap(
            long,
            default_value = "10",
            parse(try_from_str = duration_from_seconds),
        )]
    pub http_timeout: Duration,

    /// Which gas estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators support different networks.
    /// `EthGasStation`: supports mainnet.
    /// `GasNow`: supports mainnet.
    /// `GnosisSafe`: supports mainnet and rinkeby.
    /// `Web3`: supports every network.
    #[clap(
        long,
        env,
        default_value = "Web3",
        arg_enum,
        ignore_case = true,
        use_value_delimiter = true
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// BlockNative requires api key to work. Optional since BlockNative could be skipped in gas estimators.
    #[clap(long, env)]
    pub blocknative_api_key: Option<String>,

    /// Base tokens used for finding multi-hop paths between multiple AMMs
    /// Should be the most liquid tokens of the given network.
    #[clap(long, env, use_value_delimiter = true)]
    pub base_tokens: Vec<H160>,

    /// Which Liquidity sources to be used by Price Estimator.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub baseline_sources: Option<Vec<BaselineSource>>,

    /// The number of blocks kept in the pool cache.
    #[clap(long, env, default_value = "10")]
    pub pool_cache_blocks: NonZeroU64,

    /// The number of pairs that are automatically updated in the pool cache.
    #[clap(long, env, default_value = "4")]
    pub pool_cache_maximum_recent_block_age: u64,

    /// How often to retry requests in the pool cache.
    #[clap(long, env, default_value = "5")]
    pub pool_cache_maximum_retries: u32,

    /// How long to sleep in seconds between retries in the pool cache.
    #[clap(long, env, default_value = "1", parse(try_from_str = duration_from_seconds))]
    pub pool_cache_delay_between_retries_seconds: Duration,

    /// How often in seconds we poll the node to check if the current block has changed.
    #[clap(
        long,
        env,
        default_value = "5",
        parse(try_from_str = duration_from_seconds),
    )]
    pub block_stream_poll_interval_seconds: Duration,

    /// Special partner authentication for Paraswap API (allowing higher rater limits)
    #[clap(long, env)]
    pub paraswap_partner: Option<String>,

    /// The list of disabled ParaSwap DEXs. By default, the `ParaSwapPool4`
    /// DEX (representing a private market maker) is disabled as it increases
    /// price by 1% if built transactions don't actually get executed.
    #[clap(long, env, default_value = "ParaSwapPool4", use_value_delimiter = true)]
    pub disabled_paraswap_dexs: Vec<String>,

    /// Configures the back off strategy for the paraswap API when our requests get rate limited.
    /// Requests issued while back off is active get dropped entirely.
    /// Needs to be passed as "<back_off_growth_factor>,<min_back_off>,<max_back_off>".
    /// back_off_growth_factor: f64 > 1.0
    /// min_back_off: f64 in seconds
    /// max_back_off: f64 in seconds
    #[clap(long, env, verbatim_doc_comment)]
    pub paraswap_rate_limiter: Option<RateLimitingStrategy>,

    #[clap(long, env)]
    pub zeroex_url: Option<String>,

    #[clap(long, env)]
    pub zeroex_api_key: Option<String>,

    /// If quasimodo should use internal buffers to improve solution quality.
    #[clap(long, env)]
    pub quasimodo_uses_internal_buffers: bool,

    /// If mipsolver should use internal buffers to improve solution quality.
    #[clap(long, env)]
    pub mip_uses_internal_buffers: bool,

    /// The Balancer V2 factories to consider for indexing liquidity. Allows
    /// specific pool kinds to be disabled via configuration. Will use all
    /// supported Balancer V2 factory kinds if not specified.
    #[clap(long, env, arg_enum, ignore_case = true, use_value_delimiter = true)]
    pub balancer_factories: Option<Vec<BalancerFactoryKind>>,

    /// The list of disabled 1Inch protocols. By default, the `PMM1` protocol
    /// (representing a private market maker) is disabled as it seems to
    /// produce invalid swaps.
    #[clap(long, env, default_value = "PMM1", use_value_delimiter = true)]
    pub disabled_one_inch_protocols: Vec<String>,

    /// The 1Inch REST API URL to use.
    #[structopt(long, env, default_value = "https://api.1inch.exchange/")]
    pub one_inch_url: Url,

    /// The list of disabled 0x sources.
    #[clap(long, env, use_value_delimiter = true)]
    pub disabled_zeroex_sources: Vec<String>,

    /// Deny list of balancer pool ids.
    #[clap(long, env, use_value_delimiter = true)]
    pub balancer_pool_deny_list: Vec<H256>,
}

pub fn parse_unbounded_factor(s: &str) -> Result<f64> {
    let factor = f64::from_str(s)?;
    ensure!(factor.is_finite() && factor >= 0.);
    Ok(factor)
}

pub fn parse_percentage_factor(s: &str) -> Result<f64> {
    let percentage_factor = f64::from_str(s)?;
    ensure!(percentage_factor.is_finite() && percentage_factor >= 0. && percentage_factor <= 1.0);
    Ok(percentage_factor)
}

pub fn duration_from_seconds(s: &str) -> Result<Duration, ParseFloatError> {
    Ok(Duration::from_secs_f32(s.parse()?))
}

pub fn wei_from_base_unit(s: &str) -> anyhow::Result<U256> {
    Ok(U256::from_dec_str(s)? * U256::exp10(18))
}

pub fn wei_from_gwei(s: &str) -> anyhow::Result<f64> {
    let in_gwei: f64 = s.parse()?;
    Ok(in_gwei * 1e9)
}

impl FromStr for RateLimitingStrategy {
    type Err = anyhow::Error;

    fn from_str(config: &str) -> Result<Self> {
        let mut parts = config.split(',');
        let back_off_growth_factor = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing back_off_growth_factor"))?;
        let min_back_off = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing min_back_off"))?;
        let max_back_off = parts
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing max_back_off"))?;
        ensure!(
            parts.next().is_none(),
            "extraneous rate limiting parameters"
        );
        let back_off_growth_factor: f64 = back_off_growth_factor
            .parse()
            .context("parsing back_off_growth_factor")?;
        let min_back_off = duration_from_seconds(min_back_off).context("parsing min_back_off")?;
        let max_back_off = duration_from_seconds(max_back_off).context("parsing max_back_off")?;
        Self::try_new(
            back_off_growth_factor,
            min_back_off,
            max_back_off,
            "paraswap".into(),
        )
    }
}
