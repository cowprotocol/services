use {
    crate::{
        domain::Mempools,
        infra::{
            self,
            blockchain::{self, Ethereum},
            cli,
            config,
            liquidity,
            simulator::{self, Simulator},
            solver::Solver,
            Api,
        },
    },
    clap::Parser,
    futures::future::join_all,
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
    crate::infra::observe::init(&args.log);

    let ethrpc = ethrpc(&args).await;
    let web3 = ethrpc.web3().clone();
    let config = config::file::load(ethrpc.chain(), &args.config).await;
    tracing::info!("running driver with {config:#?}");

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let eth = ethereum(&config, ethrpc).await;
    let serve = Api {
        solvers: solvers(&config, &eth).await,
        liquidity: liquidity(&config, &eth).await,
        simulator: simulator(&config, &eth),
        mempools: Mempools::new(
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
        eth,
        addr: args.addr,
        addr_sender,
    }
    .serve(
        async {
            let _ = shutdown_receiver.await;
        },
        config.order_priority_strategies,
        config.settle_queue_size,
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
    blockchain::Rpc::new(&args.ethrpc)
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
            .map(|config| async move { Solver::new(config.clone(), eth.clone()).await.unwrap() })
            .collect::<Vec<_>>(),
    )
    .await
}

async fn liquidity(config: &config::Config, eth: &Ethereum) -> liquidity::Fetcher {
    liquidity::Fetcher::new(eth, &config.liquidity)
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
