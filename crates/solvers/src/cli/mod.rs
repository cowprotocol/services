//! CLI arguments for the `solvers` binary.

pub mod baseline;

use clap::{Args, Parser, Subcommand};
use std::net::SocketAddr;

/// Run a solver engine
#[derive(Parser, Debug)]
#[command(version)]
pub struct Cli {
    /// The log filter.
    #[arg(long, env, default_value = "debug")]
    pub log: String,

    #[command(flatten)]
    pub arguments: Arguments,

    #[command(subcommand)]
    pub command: Command,
}

/// Shared solver engine arguments.
#[derive(Args, Debug)]
pub struct Arguments {
    /// The socket address to bind to.
    #[arg(long, env, default_value = "127.0.0.1:7872")]
    pub addr: SocketAddr,
}

/// The solver engine command to run.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Baseline solver.
    Baseline(baseline::Arguments),
}
