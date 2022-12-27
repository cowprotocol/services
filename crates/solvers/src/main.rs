// TODO remove this once the crate stabilizes a bit.
#![allow(dead_code)]

use tokio::signal::unix::{self, SignalKind};

mod api;
mod util;

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    api::Api {
        addr: "127.0.0.1:7872".parse().unwrap(),
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
