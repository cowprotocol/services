//! Contains command line arguments and related helpers that are shared between the binaries.
use crate::gas_price_estimation::GasEstimatorType;
use ethcontract::H160;
use std::{num::ParseFloatError, time::Duration};
use url::Url;

#[derive(Debug, structopt::StructOpt)]
pub struct Arguments {
    #[structopt(
        long,
        env = "LOG_FILTER",
        default_value = "warn,orderbook=debug,solver=debug"
    )]
    pub log_filter: String,

    /// The Ethereum node URL to connect to.
    #[structopt(long, env = "NODE_URL", default_value = "http://localhost:8545")]
    pub node_url: Url,

    /// Timeout for web3 operations on the node in seconds.
    #[structopt(
            long,
            env = "NODE_TIMEOUT",
            default_value = "10",
            parse(try_from_str = duration_from_seconds),
        )]
    pub node_timeout: Duration,

    /// Which gas estimators to use. Multiple estimators are used in sequence if a previous one
    /// fails. Individual estimators support different networks.
    /// `EthGasStation`: supports mainnet.
    /// `GasNow`: supports mainnet.
    /// `GnosisSafe`: supports mainnet and rinkeby.
    /// `Web3`: supports every network.
    #[structopt(
        long,
        env = "GAS_ESTIMATORS",
        default_value = "Web3",
        possible_values = &GasEstimatorType::variants(),
        case_insensitive = true,
        use_delimiter = true
    )]
    pub gas_estimators: Vec<GasEstimatorType>,

    /// Base tokens used for finding multi-hop paths between multiple AMMs
    /// Should be the most liquid tokens of the given network.
    #[structopt(long, env = "BASE_TOKENS", use_delimiter = true)]
    pub base_tokens: Vec<H160>,
}

pub fn duration_from_seconds(s: &str) -> Result<Duration, ParseFloatError> {
    Ok(Duration::from_secs_f32(s.parse()?))
}
