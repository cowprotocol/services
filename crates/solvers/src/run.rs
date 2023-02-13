#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {crate::domain::baseline, clap::Parser, std::net::SocketAddr, tokio::sync::oneshot};

pub async fn run(args: impl Iterator<Item = String>, bind: Option<oneshot::Sender<SocketAddr>>) {
    let args = crate::cli::Args::parse_from(args);
    crate::boundary::initialize_tracing(&args.log);
    tracing::info!("running solver engine with {args:#?}");

    // TODO In the future, should use different load methods based on the command
    // being executed
    let crate::cli::Command::Baseline = args.command;
    let baseline = crate::config::baseline::file::load(&args.config).await;
    crate::api::Api {
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
