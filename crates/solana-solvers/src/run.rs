//! Binary entry: parse args, initialize observability, dispatch to the engine.

use {
    crate::{
        api::Api,
        cli::{Args, Command},
        config,
        dex,
    },
    clap::Parser,
    std::sync::Arc,
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
            let jupiter = dex::jupiter::Jupiter::new(&config.dex)
                .unwrap_or_else(|err| panic!("build jupiter dex: {err}"));
            let api = Api {
                addr: args.addr,
                dex: Arc::new(dex::Dex::Jupiter(jupiter)),
            };
            if let Err(err) = api.serve(observe::shutdown::shutdown_signal()).await {
                tracing::error!(?err, "server error");
            }
        }
    }
}
