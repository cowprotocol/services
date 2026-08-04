//! Measures what `Multicall3` batching buys the autopilot's balance fetching,
//! against a real node and the real working set.
//!
//! The benchmark drives the production `BalanceFetching` implementation through
//! the production `ethrpc` provider stack, so the numbers include the JSON-RPC
//! batching layer and the individual-call fallback.

mod bench;
mod dump;
mod fixture;
mod metrics;
mod report;
mod results;

use {anyhow::Result, clap::Parser, observe::config::Config};

#[derive(Debug, Parser)]
#[clap(about, long_about = None)]
enum Command {
    /// Dump the open-order working set from a protocol database replica.
    Dump(dump::Args),
    /// Replay a dumped working set against a node.
    Run(Box<bench::Args>),
}

#[tokio::main]
async fn main() -> Result<()> {
    // `warn` so that the fallbacks inside the balance fetcher (no `Multicall3`
    // on the chain, a whole chunk failing) show up in the output.
    observe::tracing::init::initialize_reentrant(&Config::default().with_env_filter("warn"));

    match Command::parse() {
        Command::Dump(args) => dump::run(args).await,
        Command::Run(args) => bench::run(*args).await,
    }
}
