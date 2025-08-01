use {
    crate::{
        domain::{
            Mempools,
            competition::{bad_tokens, order::app_data::AppDataRetriever},
        },
        infra::{
            self,
            Api,
            blockchain::{self, Ethereum},
            cli,
            config,
            liquidity,
            simulator::{self, Simulator},
            solver::Solver,
        },
    },
    clap::Parser,
    futures::future::join_all,
    shared::arguments::tracing_config,
    std::{net::SocketAddr, sync::Arc, time::Duration},
    tokio::sync::oneshot,
};

/// The driver entry-point. This function exists in order to be able to run the
/// driver from multiple binaries.
pub async fn start(args: impl Iterator<Item = String>) {
    observe::panic_hook::install();
    let args = cli::Args::parse_from(args);
    run_with(args, None).await
}

/// This function exists to enable running the driver for testing. The
/// `addr_sender` parameter is used so that the testing framework can get the
/// address of the server and connect to it. Outside the test suite, the
/// `addr_sender` parameter is unused. The `now` parameter allows the current
/// time to be faked for testing purposes.
pub async fn run(
    args: impl Iterator<Item = String>,
    addr_sender: Option<oneshot::Sender<SocketAddr>>,
) {
    let args = cli::Args::parse_from(args);
    run_with(args, addr_sender).await;
}

/// Run the driver. This function exists to avoid multiple monomorphizations of
/// the `run` code, which bloats the binaries and increases compile times.
async fn run_with(args: cli::Args, addr_sender: Option<oneshot::Sender<SocketAddr>>) {
    infra::observe::init(observe::Config::new(
        &args.log,
        args.stderr_threshold,
        args.use_json_logs,
        tracing_config(&args.tracing, "driver".into()),
    ));

    let ethrpc = ethrpc(&args).await;
    let web3 = ethrpc.web3().clone();
    let config = config::file::load(ethrpc.chain(), &args.config).await;
    tracing::info!("running driver with {config:#?}");

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let eth = ethereum(&config, ethrpc).await;
    let app_data_retriever = match &config.app_data_fetching {
        config::file::AppDataFetching::Enabled {
            orderbook_url,
            cache_size,
        } => Some(AppDataRetriever::new(orderbook_url.clone(), *cache_size)),
        config::file::AppDataFetching::Disabled => None,
    };
    let serve = Api {
        solvers: solvers(&config, &eth).await,
        liquidity: liquidity(&config, &eth).await,
        simulator: simulator(&config, &eth),
        mempools: Mempools::try_new(
            config
                .mempools
                .iter()
                .map(|mempool| {
                    crate::infra::mempool::Mempool::new(mempool.to_owned(), web3.clone())
                })
                .collect(),
            eth.clone(),
        )
        .unwrap(),
        bad_token_detector: bad_tokens::simulation::Detector::new(
            config.simulation_bad_token_max_age,
            &eth,
        ),
        eth,
        addr: args.addr,
        addr_sender,
    }
    .serve(
        async {
            let _ = shutdown_receiver.await;
        },
        config.order_priority_strategies,
        app_data_retriever,
    );

    futures::pin_mut!(serve);
    tokio::select! {
        result = &mut serve => panic!("serve task exited: {result:?}"),
        _ = shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            // Shutdown timeout needs to be larger than the auction deadline
            match tokio::time::timeout(Duration::from_secs(20), serve).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => panic!("API shutdown exceeded timeout"),
            }
        }
    };
}

fn simulator(config: &infra::Config, eth: &Ethereum) -> Simulator {
    let mut simulator = match &config.simulator {
        Some(infra::simulator::Config::Tenderly(tenderly)) => Simulator::tenderly(
            simulator::tenderly::Config {
                url: tenderly.url.to_owned(),
                api_key: tenderly.api_key.to_owned(),
                user: tenderly.user.to_owned(),
                project: tenderly.project.to_owned(),
                save: tenderly.save,
                save_if_fails: tenderly.save_if_fails,
            },
            eth.to_owned(),
        ),
        Some(infra::simulator::Config::Enso(enso)) => Simulator::enso(
            simulator::enso::Config {
                url: enso.url.to_owned(),
                network_block_interval: enso.network_block_interval.to_owned(),
            },
            eth.to_owned(),
        ),
        None => Simulator::ethereum(eth.to_owned()),
    };
    if config.disable_access_list_simulation {
        simulator.disable_access_lists()
    }
    if let Some(gas) = config.disable_gas_simulation {
        simulator.disable_gas(gas)
    }
    simulator
}

async fn ethrpc(args: &cli::Args) -> blockchain::Rpc {
    let args = blockchain::RpcArgs {
        url: args.ethrpc.clone(),
        max_batch_size: args.ethrpc_max_batch_size,
        max_concurrent_requests: args.ethrpc_max_concurrent_requests,
    };
    blockchain::Rpc::try_new(args)
        .await
        .expect("connect ethereum RPC")
}

async fn ethereum(config: &infra::Config, ethrpc: blockchain::Rpc) -> Ethereum {
    let gas = Arc::new(
        blockchain::GasPriceEstimator::new(ethrpc.web3(), &config.gas_estimator, &config.mempools)
            .await
            .expect("initialize gas price estimator"),
    );
    Ethereum::new(
        ethrpc,
        config.contracts.clone(),
        gas,
        config.archive_node_url.as_ref(),
    )
    .await
}

async fn solvers(config: &config::Config, eth: &Ethereum) -> Vec<Solver> {
    join_all(
        config
            .solvers
            .iter()
            .map(
                |config| async move { Solver::try_new(config.clone(), eth.clone()).await.unwrap() },
            )
            .collect::<Vec<_>>(),
    )
    .await
}

async fn liquidity(config: &config::Config, eth: &Ethereum) -> liquidity::Fetcher {
    liquidity::Fetcher::try_new(eth, &config.liquidity)
        .await
        .expect("initialize liquidity fetcher")
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept signals for graceful shutdown. Kubernetes sends sigterm, Ctrl-C
    // sends sigint.
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .unwrap()
            .recv()
            .await
    };
    let sigint = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .unwrap()
            .recv()
            .await;
    };
    futures::pin_mut!(sigint);
    futures::pin_mut!(sigterm);
    futures::future::select(sigterm, sigint).await;
}

#[cfg(windows)]
async fn shutdown_signal() {
    // No support for signal handling on Windows.
    std::future::pending().await
}
