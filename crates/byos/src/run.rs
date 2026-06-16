#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        api::{Api, AppState},
        domain::{eip712, proposal::ProposalStore},
        infra::{cli, config},
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
        tracing_config(&args.tracing, "byos".into()),
    );
    observe::tracing::init::initialize_reentrant(&obs_config);

    let commit_hash = option_env!("VERGEN_GIT_SHA").unwrap_or("COMMIT_INFO_NOT_FOUND");
    tracing::info!(%commit_hash, "running BYOS engine with {args:#?}");

    let config = config::load(&args.config).await;
    let domain = eip712::byos_domain(config.chain_id);

    tracing::info!(chain_id = config.chain_id, "BYOS configured");

    Api {
        addr: args.addr,
        state: AppState {
            store: ProposalStore::new(),
            domain,
        },
    }
    .serve(bind, shutdown_signal())
    .await
    .unwrap();
}

#[cfg(unix)]
async fn shutdown_signal() {
    let mut interrupt = unix::signal(SignalKind::interrupt()).unwrap();
    let mut terminate = unix::signal(SignalKind::terminate()).unwrap();
    tokio::select! {
        _ = interrupt.recv() => (),
        _ = terminate.recv() => (),
    };
}

#[cfg(windows)]
async fn shutdown_signal() {
    std::future::pending().await
}
