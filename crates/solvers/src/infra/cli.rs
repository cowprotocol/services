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

    #[command(subcommand)]
    pub command: Command,
}

/// The solver engine to run.
#[derive(Subcommand, Debug)]
#[clap(rename_all = "lowercase")]
pub enum Command {
    /// Baseline solver.
    Baseline {
        /// Path to the solver configuration file. This file should be in TOML
        /// format.
        #[clap(long, env)]
        config: PathBuf,
    },
    /// Naive solver.
    Naive,
    /// Wrapper for solvers implementing the legacy HTTP interface.
    Legacy {
        /// Path to the solver configuration file. This file should be in TOML
        /// format.
        #[clap(long, env)]
        config: PathBuf,
    },
    /// Balancer SOR solver.
    Balancer {
        /// Path to the solver configuration file. This file should be in TOML
        /// format.
        #[clap(long, env)]
        config: PathBuf,
    },
    /// 0x solver.
    ZeroEx {
        /// Path to the solver configuration file. This file should be in TOML
        /// format.
        #[clap(long, env)]
        config: PathBuf,
    },
}
