#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        domain::solver::{self, Solver},
        infra::{cli, config},
    },
    clap::Parser,
    std::net::SocketAddr,
    tokio::sync::oneshot,
};

pub async fn start(args: impl IntoIterator<Item = String>) {
    observe::panic_hook::install();
    let args = cli::Args::parse_from(args);
    run_with(args, None).await;
}

pub async fn run(
    args: impl IntoIterator<Item = String>,
    bind: Option<oneshot::Sender<SocketAddr>>,
) {
    let args = cli::Args::parse_from(args);
    run_with(args, bind).await;
}

async fn run_with(args: cli::Args, bind: Option<oneshot::Sender<SocketAddr>>) {
    observe::tracing::initialize_reentrant(&args.log);
    tracing::info!("running solver engine with {args:#?}");

    let solver = match args.command {
        cli::Command::Baseline { config } => {
            let config = config::baseline::load(&config).await;
            Solver::Baseline(solver::Baseline::new(config))
        }
        cli::Command::Naive => Solver::Naive(solver::Naive),
    };

    crate::api::Api {
        addr: args.addr,
        solver,
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
