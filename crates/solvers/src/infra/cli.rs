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
    #[arg(long, env, default_value = "debug")]
    pub log: String,

    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7872")]
    pub addr: SocketAddr,

    /// Path to the driver configuration file. This file should be in TOML
    /// format.
    #[clap(long, env, conflicts_with = "config_string")]
    pub config_path: Option<PathBuf>,

    /// Like `config` but reads the configuration directly from the argument
    /// instead of a file.
    #[clap(long, env, conflicts_with = "config_path")]
    pub config_string: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

/// The solver engine to run.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Baseline solver.
    Baseline,
}
