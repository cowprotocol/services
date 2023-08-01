use {
    crate::{
        boundary,
        domain::{eth, Mempools},
        infra::{
            self,
            blockchain::{self, Ethereum},
            cli,
            config,
            liquidity,
            mempool,
            observe,
            simulator::{self, Simulator},
            solver::Solver,
            Api,
            Mempool,
        },
    },
    clap::Parser,
    futures::future::join_all,
    std::{net::SocketAddr, time::Duration},
    tokio::sync::oneshot,
};

pub async fn main() {
    boundary::exit_process_on_panic::set_panic_hook();
    run(std::env::args(), None).await
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
    observe::init(&args.log);
    let config = config::file::load(&args.config).await;

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let eth = ethereum(&config, &args).await;
    let tx_pool = mempool::GlobalTxPool::default();
    let serve = Api {
        solvers: solvers(&config, &eth),
        liquidity: liquidity(&config, &eth).await,
        simulator: simulator(&config, &eth),
        mempools: Mempools::new(
            join_all(
                config
                    .mempools
                    .iter()
                    .map(|mempool| Mempool::new(mempool.to_owned(), eth.clone(), tx_pool.clone())),
            )
            .await
            .into_iter()
            .flatten()
            .collect(),
        )
        .unwrap(),
        eth,
        addr: args.addr,
        addr_sender,
    }
    .serve(async {
        let _ = shutdown_receiver.await;
    });

    futures::pin_mut!(serve);
    tokio::select! {
        result = &mut serve => panic!("serve task exited: {result:?}"),
        _ = shutdown_signal() => {
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => panic!("API shutdown exceeded timeout"),
            }
        }
    };
}

fn simulator(config: &infra::Config, eth: &Ethereum) -> Simulator {
    let mut simulator = match &config.tenderly {
        Some(tenderly) => Simulator::tenderly(
            simulator::tenderly::Config {
                url: tenderly.url.to_owned(),
                api_key: tenderly.api_key.to_owned(),
                user: tenderly.user.to_owned(),
                project: tenderly.project.to_owned(),
                save: tenderly.save,
                save_if_fails: tenderly.save_if_fails,
            },
            eth.network_id().to_owned(),
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

async fn ethereum(config: &infra::Config, args: &cli::Args) -> Ethereum {
    Ethereum::ethrpc(
        &args.ethrpc,
        blockchain::contracts::Addresses {
            settlement: config.contracts.gp_v2_settlement.map(Into::into),
            weth: config.contracts.weth.map(Into::into),
        },
    )
    .await
    .expect("initialize ethereum RPC API")
}

fn solvers(config: &config::Config, eth: &Ethereum) -> Vec<Solver> {
    config
        .solvers
        .iter()
        .map(|config| Solver::new(config.clone(), eth.clone()))
        .collect()
}

async fn liquidity(config: &config::Config, eth: &Ethereum) -> liquidity::Fetcher {
    let config = liquidity::Config {
        base_tokens: config
            .liquidity
            .base_tokens
            .iter()
            .copied()
            .map(eth::TokenAddress::from)
            .collect(),
        uniswap_v2: config
            .liquidity
            .uniswap_v2
            .iter()
            .cloned()
            .map(|config| match config {
                config::file::UniswapV2Config::Preset { preset } => match preset {
                    config::file::UniswapV2Preset::UniswapV2 => {
                        liquidity::config::UniswapV2::uniswap_v2(eth.network_id())
                    }
                    config::file::UniswapV2Preset::SushiSwap => {
                        liquidity::config::UniswapV2::sushi_swap(eth.network_id())
                    }
                    config::file::UniswapV2Preset::Honeyswap => {
                        liquidity::config::UniswapV2::honeyswap(eth.network_id())
                    }
                    config::file::UniswapV2Preset::Baoswap => {
                        liquidity::config::UniswapV2::baoswap(eth.network_id())
                    }
                    config::file::UniswapV2Preset::PancakeSwap => {
                        liquidity::config::UniswapV2::pancake_swap(eth.network_id())
                    }
                }
                .expect("no Uniswap V2 preset for current network"),
                config::file::UniswapV2Config::Manual { router, pool_code } => {
                    liquidity::config::UniswapV2 {
                        router: router.into(),
                        pool_code: pool_code.into(),
                    }
                }
            })
            .collect(),
        swapr: config
            .liquidity
            .swapr
            .iter()
            .cloned()
            .map(|config| match config {
                config::file::SwaprConfig::Preset { preset } => match preset {
                    config::file::SwaprPreset::Swapr => {
                        liquidity::config::Swapr::swapr(eth.network_id())
                    }
                }
                .expect("no Swapr preset for current network"),
                config::file::SwaprConfig::Manual { router, pool_code } => {
                    liquidity::config::Swapr {
                        router: router.into(),
                        pool_code: pool_code.into(),
                    }
                }
            })
            .collect(),
        uniswap_v3: config
            .liquidity
            .uniswap_v3
            .iter()
            .cloned()
            .map(|config| match config {
                config::file::UniswapV3Config::Preset {
                    preset,
                    max_pools_to_initialize,
                } => liquidity::config::UniswapV3 {
                    max_pools_to_initialize,
                    ..match preset {
                        config::file::UniswapV3Preset::UniswapV3 => {
                            liquidity::config::UniswapV3::uniswap_v3(eth.network_id())
                        }
                    }
                    .expect("no Uniswap V3 preset for current network")
                },
                config::file::UniswapV3Config::Manual {
                    router,
                    max_pools_to_initialize,
                } => liquidity::config::UniswapV3 {
                    router: router.into(),
                    max_pools_to_initialize,
                },
            })
            .collect(),
        balancer_v2: config
            .liquidity
            .balancer_v2
            .iter()
            .cloned()
            .map(|config| match config {
                config::file::BalancerV2Config::Preset {
                    preset,
                    pool_deny_list,
                } => liquidity::config::BalancerV2 {
                    pool_deny_list: pool_deny_list.clone(),
                    ..match preset {
                        config::file::BalancerV2Preset::BalancerV2 => {
                            liquidity::config::BalancerV2::balancer_v2(eth.network_id())
                        }
                    }
                    .expect("no Balancer V2 preset for current network")
                },
                config::file::BalancerV2Config::Manual {
                    vault,
                    weighted,
                    stable,
                    liquidity_bootstrapping,
                    pool_deny_list,
                } => liquidity::config::BalancerV2 {
                    vault: vault.into(),
                    weighted: weighted
                        .into_iter()
                        .map(eth::ContractAddress::from)
                        .collect(),
                    stable: stable.into_iter().map(eth::ContractAddress::from).collect(),
                    liquidity_bootstrapping: liquidity_bootstrapping
                        .into_iter()
                        .map(eth::ContractAddress::from)
                        .collect(),
                    pool_deny_list: pool_deny_list.clone(),
                },
            })
            .collect(),
    };
    liquidity::Fetcher::new(eth, &config)
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
