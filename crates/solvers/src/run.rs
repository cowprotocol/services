#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        domain::baseline,
        infra::{cli, config, contracts},
    },
    clap::Parser,
    std::net::SocketAddr,
    tokio::sync::oneshot,
};

pub async fn run(args: impl Iterator<Item = String>, bind: Option<oneshot::Sender<SocketAddr>>) {
    let args = cli::Args::parse_from(args);
    crate::boundary::initialize_tracing(&args.log);
    tracing::info!("running solver engine with {args:#?}");

    // TODO In the future, should use different load methods based on the command
    // being executed
    let cli::Command::Baseline = args.command;
    let baseline = match (&args.config_string, &args.config_path) {
        (Some(string), None) => config::baseline::file::load_string(string),
        (None, Some(path)) => config::baseline::file::load_path(path).await,
        (None, None) => panic!("specify --config-string or --config-path"),
        (Some(_), Some(_)) => unreachable!(),
    };
    let contracts = contracts::Contracts::new(
        baseline.chain_id,
        contracts::Addresses {
            weth: baseline.weth,
        },
    );
    crate::api::Api {
        addr: args.addr,
        solver: baseline::Baseline {
            weth: *contracts.weth(),
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
