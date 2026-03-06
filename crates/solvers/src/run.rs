#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        domain::solver,
        infra::{cli, config, dex},
    },
    clap::Parser,
    shared::arguments::tracing_config,
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
    let obs_config = observe::Config::new(
        &args.log,
        tracing::Level::ERROR.into(),
        args.use_json_logs,
        tracing_config(&args.tracing, "solvers".into()),
    );
    observe::tracing::init::initialize_reentrant(&obs_config);
    #[cfg(all(unix, feature = "jemalloc"))]
    observe::heap_dump_handler::spawn_heap_dump_handler();

    let commit_hash = option_env!("VERGEN_GIT_SHA").unwrap_or("COMMIT_INFO_NOT_FOUND");

    tracing::info!(%commit_hash, "running solver engine with {args:#?}");

    let solver = match args.command {
        cli::Command::Baseline { config: path } => {
            let config = config::baseline::load(&path).await;
            solver::Solver::Baseline(solver::Baseline::new(config).await)
        }
        cli::Command::Okx { config: path } => {
            let config = config::dex::okx::file::load(&path).await;
            solver::Solver::Dex(Box::new(solver::Dex::new(
                dex::Dex::Okx(Box::new(
                    dex::okx::Okx::try_new(config.okx).expect("invalid OKX configuration"),
                )),
                config.base,
            )))
        }
        cli::Command::Bitget { config: path } => {
            let config = config::dex::bitget::file::load(&path).await;
            solver::Solver::Dex(Box::new(solver::Dex::new(
                dex::Dex::Bitget(
                    dex::bitget::Bitget::try_new(config.bitget)
                        .expect("invalid Bitget configuration"),
                ),
                config.base,
            )))
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
