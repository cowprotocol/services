//! Contains command line arguments and related helpers that are shared between the binaries.

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
}

pub fn duration_from_seconds(s: &str) -> Result<Duration, ParseFloatError> {
    Ok(Duration::from_secs_f32(s.parse()?))
}
