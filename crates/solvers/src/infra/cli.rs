//! CLI arguments for the `solvers` binary.

use {
    crate::domain::eth,
    clap::{Parser, Subcommand},
    std::{net::SocketAddr, path::PathBuf},
};

/// Run a solver engine
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// The log filter.
    #[arg(long, env, default_value = "debug")]
    pub log: String,

    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7872")]
    pub addr: SocketAddr,

    /// The chain ID this solver is for.
    #[arg(long, env, value_parser = parse_chain_id)]
    pub chain_id: eth::ChainId,

    /// Path to the driver configuration file. This file should be in TOML
    /// format.
    #[clap(long, env)]
    pub config: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

/// The solver engine to run.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Baseline solver.
    Baseline,
}

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

fn parse_chain_id(arg: &str) -> Result<eth::ChainId, Error> {
    Ok(eth::ChainId::new(arg.parse()?)?)
}
