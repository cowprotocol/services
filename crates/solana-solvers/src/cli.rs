//! CLI arguments for the `solana-solvers` binary.

use {
    clap::{Parser, Subcommand},
    std::{net::SocketAddr, path::PathBuf},
};

/// Run a Solana solver engine.
#[derive(Parser, Debug)]
#[command(version)]
pub struct Args {
    /// The log filter.
    #[arg(long, env, default_value = "warn,solana_solvers=debug")]
    pub log: String,

    /// Whether to use JSON format for the logs.
    #[clap(long, env, default_value = "false")]
    pub use_json_logs: bool,

    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7900")]
    pub addr: SocketAddr,

    #[command(subcommand)]
    pub command: Command,
}

/// The solver engine to run. `config` is a path to a TOML config file.
#[derive(Subcommand, Debug)]
#[clap(rename_all = "lowercase")]
pub enum Command {
    /// Wrap Jupiter's quote API into single-order solutions.
    Jupiter {
        #[clap(long, env)]
        config: PathBuf,
    },
    // TODO: add a baseline engine subcommand.
}
