//! CLI arguments for the `solvers` binary.

use {
    crate::boundary::rate_limiter::RateLimitingStrategy,
    clap::{Parser, Subcommand},
    std::{net::SocketAddr, path::PathBuf},
};

/// Run a solver engine
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// The log filter.
    #[arg(
        long,
        env,
        default_value = "warn,solvers=debug,shared=debug,model=debug,solver=debug"
    )]
    pub log: String,

    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7872")]
    pub addr: SocketAddr,

    #[command(subcommand)]
    pub command: Command,

    /// Configures the back off strategy for single order solvers. Requests
    /// issued while back off is active get dropped entirely. Expects
    /// "<factor >= 1.0>,<min: seconds>,<max: seconds>".
    #[clap(long, env)]
    pub single_order_solver_rate_limiter: Option<RateLimitingStrategy>,
}

/// The solver engine to run. The config field is a path to the solver
/// configuration file. This file should be in TOML format.
#[derive(Subcommand, Debug)]
#[clap(rename_all = "lowercase")]
pub enum Command {
    /// solve individual orders exclusively via provided onchain liquidity
    Baseline {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// optimistically batch similar orders and get difference from AMMs
    Naive {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// forward auction to solver implementing the legacy HTTP interface
    Legacy {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using Balancer API
    Balancer {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using 0x API
    ZeroEx {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using 1Inch API
    OneInch {
        #[clap(long, env)]
        config: PathBuf,
    },
    /// solve individual orders using Paraswap API
    ParaSwap {
        #[clap(long, env)]
        config: PathBuf,
    },
}

impl Command {
    pub fn to_lowercase(&self) -> String {
        match self {
            Command::Baseline { .. } => "baseline".to_string(),
            Command::Naive { .. } => "naive".to_string(),
            Command::Legacy { .. } => "legacy".to_string(),
            Command::Balancer { .. } => "balancer".to_string(),
            Command::ZeroEx { .. } => "zeroex".to_string(),
            Command::OneInch { .. } => "oneinch".to_string(),
            Command::ParaSwap { .. } => "paraswap".to_string(),
        }
    }
}
