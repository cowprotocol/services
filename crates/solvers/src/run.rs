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
    #[cfg(unix)]
    observe::heap_dump_handler::spawn_heap_dump_handler();

    let version = observe::version::git_version();

    tracing::info!(%version, "running solver engine with {args:#?}");

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
    .serve(bind, observe::shutdown::shutdown_signal())
    .await
    .unwrap();
}
