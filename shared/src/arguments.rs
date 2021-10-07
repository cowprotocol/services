//! Contains command line arguments and related helpers that are shared between the binaries.
use crate::{
    gas_price_estimation::GasEstimatorType, price_estimation::PriceEstimatorType,
    sources::BaselineSource,
};
use anyhow::{ensure, Result};
use ethcontract::{H160, U256};
use std::{
    num::{NonZeroU64, ParseFloatError},
    str::FromStr,
    time::Duration,
};
use url::Url;

#[derive(Debug, structopt::StructOpt)]
pub struct Arguments {
    #[structopt(
        long,
        env,
        default_value = "warn,orderbook=debug,solver=debug,shared=debug,shared::transport::http=info,archerapi=info"
    )]
    pub log_filter: String,

    /// The Ethereum node URL to connect to.
    #[structopt(long, env, default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// Timeout in seconds for all http requests.
    #[structopt(
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
    #[structopt(
        long,
        env,
        default_value = "Web3",
        possible_values = &GasEstimatorType::variants(),
        case_insensitive = true,
        use_delimiter = true
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// Base tokens used for finding multi-hop paths between multiple AMMs
    /// Should be the most liquid tokens of the given network.
    #[structopt(long, env, use_delimiter = true)]
    pub base_tokens: Vec<H160>,

    /// Gas Fee Factor: 1.0 means cost is forwarded to users alteration, 0.9 means there is a 10%
    /// subsidy, 1.1 means users pay 10% in fees than what we estimate we pay for gas.
    #[structopt(long, env, default_value = "1", parse(try_from_str = parse_fee_factor))]
    pub fee_factor: f64,

    /// Which Liquidity sources to be used by Price Estimator.
    #[structopt(
        long,
        env,
        default_value = "Uniswap,Sushiswap",
        possible_values = &BaselineSource::variants(),
        case_insensitive = true,
        use_delimiter = true
    )]
    pub baseline_sources: Vec<BaselineSource>,

    /// The number of blocks kept in the pool cache.
    #[structopt(long, env, default_value = "10")]
    pub pool_cache_blocks: NonZeroU64,

    /// The number of pairs that are automatically updated in the pool cache.
    #[structopt(long, env, default_value = "4")]
    pub pool_cache_maximum_recent_block_age: u64,

    /// How often to retry requests in the pool cache.
    #[structopt(long, env, default_value = "5")]
    pub pool_cache_maximum_retries: u32,

    /// How long to sleep in seconds between retries in the pool cache.
    #[structopt(long, env, default_value = "1", parse(try_from_str = duration_from_seconds))]
    pub pool_cache_delay_between_retries_seconds: Duration,

    /// How often in seconds we poll the node to check if the current block has changed.
    #[structopt(
        long,
        env,
        default_value = "5",
        parse(try_from_str = duration_from_seconds),
    )]
    pub block_stream_poll_interval_seconds: Duration,

    /// The amount in native tokens atoms to use for price estimation. Should be reasonably large so
    // that small pools do not influence the prices. If not set a reasonable default is used based
    // on network id.
    #[structopt(
        long,
        env,
        parse(try_from_str = U256::from_dec_str)
    )]
    pub amount_to_estimate_prices_with: Option<U256>,

    /// Special partner authentication for Paraswap API (allowing higher rater limits)
    #[structopt(long, env)]
    pub paraswap_partner: Option<String>,

    /// The list of disabled ParaSwap DEXs. By default, the `ParaSwapPool4`
    /// DEX (representing a private market maker) is disabled as it increases
    /// price by 1% if built transactions don't actually get executed.
    #[structopt(long, env, default_value = "ParaSwapPool4", use_delimiter = true)]
    pub disabled_paraswap_dexs: Vec<String>,

    #[structopt(
        long,
        env,
        default_value = "Baseline",
        possible_values = &PriceEstimatorType::variants(),
        use_delimiter = true
    )]
    pub price_estimators: Vec<PriceEstimatorType>,
}

fn parse_fee_factor(s: &str) -> Result<f64> {
    let f64 = f64::from_str(s)?;
    ensure!(f64.is_finite() && f64 >= 0.);
    Ok(f64)
}

pub fn duration_from_seconds(s: &str) -> Result<Duration, ParseFloatError> {
    Ok(Duration::from_secs_f32(s.parse()?))
}

pub fn wei_from_base_unit(s: &str) -> anyhow::Result<U256> {
    Ok(U256::from_dec_str(s)? * U256::exp10(18))
}

pub fn wei_from_gwei(s: &str) -> anyhow::Result<f64> {
    let in_gwei: f64 = s.parse()?;
    Ok(in_gwei * 10e9)
}

pub fn default_amount_to_estimate_prices_with(network_id: &str) -> Option<U256> {
    match network_id {
        // Mainnet, Rinkeby
        "1" | "4" => Some(10u128.pow(18).into()),
        // Xdai
        "100" => Some(10u128.pow(21).into()),
        _ => None,
    }
}
