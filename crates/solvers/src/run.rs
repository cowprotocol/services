#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        domain::solver::{self, Solver},
        infra::{cli, config, dex},
    },
    clap::Parser,
    std::net::SocketAddr,
    tokio::sync::oneshot,
};

pub async fn run(
    args: impl IntoIterator<Item = String>,
    bind: Option<oneshot::Sender<SocketAddr>>,
) {
    let args = cli::Args::parse_from(args);
    crate::boundary::initialize_tracing(&args.log);
    tracing::info!("running solver engine with {args:#?}");

    let solver = match args.command {
        cli::Command::Baseline { config } => {
            let baseline = config::baseline::file::load(&config).await;
            Solver::Baseline(solver::Baseline {
                weth: baseline.weth,
                base_tokens: baseline.base_tokens.into_iter().collect(),
                max_hops: baseline.max_hops,
                max_partial_attempts: baseline.max_partial_attempts,
            })
        }
        cli::Command::Naive => Solver::Naive(solver::Naive),
        cli::Command::Legacy { config } => {
            let config = config::legacy::load(&config).await;
            Solver::Legacy(solver::Legacy::new(config))
        }
        cli::Command::ZeroEx { config } => {
            let config = config::dex::zeroex::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::ZeroEx(
                    dex::zeroex::ZeroEx::new(config.zeroex).expect("invalid 0x configuration"),
                ),
                config.base,
            ))
        }
        cli::Command::Balancer { config } => {
            let config = config::dex::balancer::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::Balancer(dex::balancer::Sor::new(config.sor)),
                config.base,
            ))
        }
        cli::Command::OneInch { config } => {
            let config = config::dex::oneinch::file::load(&config).await;
            Solver::Dex(solver::Dex::new(
                dex::Dex::OneInch(dex::oneinch::OneInch::new(config.oneinch).await.unwrap()),
                config.base,
            ))
        }
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
