// TODO remove this once the crate stabilizes a bit.
#![allow(dead_code)]

mod api;
mod boundary;
mod cli;
mod config;
mod domain;
#[cfg(test)]
mod tests;
mod util;

use crate::domain::baseline;
use clap::Parser;
use std::net::SocketAddr;
#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    boundary::exit_process_on_panic::set_panic_hook();

    // TODO implement Display for the arguments
    run(std::env::args(), None).await;
}

async fn run(args: impl Iterator<Item = String>, bind: Option<oneshot::Sender<SocketAddr>>) {
    let args = cli::Args::parse_from(args);
    boundary::initialize_tracing(&args.log);
    tracing::info!("running solver engine with {args:#?}");

    // TODO In the future, should use different load methods based on the command being executed
    let cli::Command::Baseline = args.command;
    let baseline = config::baseline::file::load(&args.config).await;
    api::Api {
        addr: args.addr,
        solver: baseline::Baseline {
            weth: baseline.weth,
            base_tokens: baseline.base_tokens.into_iter().collect(),
            max_hops: baseline.max_hops,
        },
    }
    .serve(bind, shutdown_signal())
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
