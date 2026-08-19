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

/// Default filter. `warn` so that the fallbacks inside the balance fetcher (no
/// `Multicall3` on the chain, a whole chunk failing) show up in the output,
/// without the per-batch chatter drowning the table.
const DEFAULT_LOG_FILTER: &str = "warn";

/// Raise this to watch batching happen call by call. The summary the benchmark
/// prints comes from counters, not from these logs, so this is only for looking
/// at individual packets:
///
/// ```text
/// LOG_FILTER=warn,ethrpc::alloy::buffering=debug,account_balances=debug
/// ```
const LOG_FILTER: &str = "LOG_FILTER";

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var(LOG_FILTER).unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_owned());
    observe::tracing::init::initialize_reentrant(&Config::default().with_env_filter(&filter));

    match Command::parse() {
        Command::Dump(args) => dump::run(args).await,
        Command::Run(args) => bench::run(*args).await,
    }
}
