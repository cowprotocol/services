//! Binary entry: parse args, initialize observability, dispatch to the engine.

#[cfg(unix)]
use tokio::signal::unix::{self, SignalKind};
use {
    crate::{
        api::Api,
        cli::{Args, Command},
        config,
    },
    clap::Parser,
};

/// Parse args and run the selected solver engine until shutdown.
pub async fn start(args: impl IntoIterator<Item = String>) {
    observe::panic_hook::install();
    let args = Args::parse_from(args);

    let obs_config = observe::Config::new(
        &args.log,
        Some(tracing::Level::ERROR),
        args.use_json_logs,
        None,
    );
    observe::tracing::init::initialize_reentrant(&obs_config);
    tracing::info!(version = %observe::version::git_version(), "running solana-solvers with {args:#?}");

    match args.command {
        Command::Jupiter { config: path } => {
            let config = config::load(&path).await;
            let api = Api {
                addr: args.addr,
                config,
            };
            if let Err(err) = api.serve(shutdown_signal()).await {
                tracing::error!(?err, "server error");
            }
        }
    }
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Kubernetes sends SIGTERM; locally SIGINT (ctrl-c) is most common.
    let mut interrupt = unix::signal(SignalKind::interrupt()).expect("install SIGINT handler");
    let mut terminate = unix::signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = interrupt.recv() => (),
        _ = terminate.recv() => (),
    };
}

#[cfg(windows)]
async fn shutdown_signal() {
    // Signal handling is not supported on Windows.
    std::future::pending().await
}
