//! CLI arguments for the `solvers` binary.

use {
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
}
