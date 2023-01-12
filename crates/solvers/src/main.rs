// TODO remove this once the crate stabilizes a bit.
#![allow(dead_code)]

mod api;
mod boundary;
mod cli;
mod domain;
mod util;

use crate::domain::baseline;
use clap::Parser;
#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use tracing::level_filters::LevelFilter;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();

    shared::tracing::initialize(&cli.log_filter, LevelFilter::ERROR);
    shared::exit_process_on_panic::set_panic_hook();

    // TODO implement Display for the arguments
    tracing::info!("running solver engine with {cli:#?}");
    run(cli.arguments, cli.command).await;
}

async fn run(arguments: cli::Arguments, command: cli::Command) {
    let cli::Command::Baseline(baseline) = command;

    api::Api {
        addr: arguments.addr,
        solver: baseline::Baseline {
            weth: baseline.weth,
            base_tokens: baseline.base_tokens.into_iter().collect(),
            max_hops: baseline.max_hops,
        },
    }
    .serve(shutdown_signal())
    .await
    .unwrap();
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown.
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common.
    let mut interrupt = unix::signal(SignalKind::interrupt()).unwrap();
    let mut terminate = unix::signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = interrupt.recv() => (),
        _ = terminate.recv() => (),
    };
}

#[cfg(windows)]
async fn shutdown_signal() {
    // We don't support signal handling on Windows.
    std::future::pending().await
}
