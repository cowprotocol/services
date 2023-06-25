//! This crate provides a single binary entrypoint for all services. This allows
//! for significantly smaller Docker images for deployments.

use {clap::Parser, std::env};

#[derive(Debug, Parser)]
enum Binary {
    /// Monitor the orderbook and record metrics for monitoring services health.
    Alerter,
    /// Cut and execute auctions.
    Autopilot,
    /// Calculate quote and solve auctions.
    Driver,
    /// External API for user interactions with the protocol.
    Orderbook,
    /// EthFlow expired order refunding service.
    Refunder,
    /// Built-in solving engines.
    Solvers,
    /// (LEGACY) Solve auctions.
    Solver,
}

#[tokio::main]
async fn main() {
    let binary = Binary::parse_from(env::args().take(2));
    let args = env::args().skip(1);

    match binary {
        Binary::Alerter => alerter::start(args).await,
        Binary::Autopilot => autopilot::start(args).await,
        Binary::Driver => driver::start(args).await,
        Binary::Orderbook => orderbook::start(args).await,
        Binary::Refunder => refunder::start(args).await,
        Binary::Solvers => solvers::start(args).await,
        Binary::Solver => solver::start(args).await,
    };
}
