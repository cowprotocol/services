// TODO Remove dead_code ASAP
#![allow(dead_code)]
#![forbid(unsafe_code)]

use {
    crate::{
        domain::competition,
        infra::{api, mempool, Mempool},
    },
    futures::future::join_all,
    infra::blockchain,
    std::net::SocketAddr,
    tokio::sync::oneshot,
};

mod boundary;
mod domain;
mod infra;
mod util;

#[cfg(test)]
mod tests;

use {
    clap::Parser,
    config::cli,
    infra::{
        blockchain::Ethereum,
        config,
        simulator::{self, Simulator},
        solver::{self, Solver},
        Api,
    },
    std::time::Duration,
    tracing::level_filters::LevelFilter,
};

#[tokio::main]
async fn main() {
    // TODO This should be set based on the CLI args, so it can't be here. I need to
    // find a neat way to solve this.
    boundary::initialize_tracing(
        "debug,hyper=warn,driver::infra::solver=trace",
        LevelFilter::ERROR,
    );
    run(std::env::args(), infra::time::Now::Real, None).await
}

/// This function exists to enable running the driver for testing. The
/// `addr_sender` parameter is used so that the testing framework can get the
/// address of the server and connect to it. Outside the test suite, the
/// `addr_sender` parameter is unused. The `now` parameter allows the current
/// time to be faked for testing purposes.
pub async fn run(
    args: impl Iterator<Item = String>,
    now: infra::time::Now,
    addr_sender: Option<oneshot::Sender<SocketAddr>>,
) {
    let args = cli::Args::parse_from(args);
    boundary::exit_process_on_panic::set_panic_hook();
    tracing::info!("running driver with arguments:\n{}", args);

    let tx_pool = mempool::GlobalTxPool::default();

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let eth = ethereum(&args).await;
    let config = mempool::Config {
        additional_tip_percentage: args.submission.submission_additional_tip_percentage,
        max_additional_tip: None,
        gas_price_cap: args.submission.submission_gas_price_cap,
        target_confirm_time: std::time::Duration::from_secs(
            args.submission.submission_target_confirm_time_secs,
        ),
        max_confirm_time: std::time::Duration::from_secs(
            args.submission.submission_max_confirm_time_secs,
        ),
        retry_interval: std::time::Duration::from_secs(
            args.submission.submission_retry_interval_secs,
        ),
        account: match (args.solver_address, args.solver_private_key.clone()) {
            (Some(address), None) => ethcontract::Account::Local(address, None),
            (None, Some(private_key)) => ethcontract::Account::Offline(
                ethcontract::PrivateKey::from_hex_str(private_key)
                    .expect("a valid private key in --solver-private-key"),
                None,
            ),
            _ => panic!("exactly one of --solver-address, --solver-private-key must be specified",),
        },
        eth: eth.clone(),
        pool: tx_pool.clone(),
    };
    let gas_price_estimator = mempool::gas_price_estimator(&config).await.unwrap();
    let serve = Api {
        solvers: solvers(&args, now).await,
        simulator: simulator(&args, &eth),
        mempools: join_all(args.mempools.iter().map(|&mempool| {
            let args = &args;
            let config = config.clone();
            let gas_price_estimator = gas_price_estimator.clone();
            async move {
                match mempool {
                    cli::Mempool::Public => vec![Mempool::public(
                        config,
                        if args
                            .submission
                            .submission_disable_high_risk_public_mempool_transactions
                        {
                            mempool::HighRisk::Disabled
                        } else {
                            mempool::HighRisk::Enabled
                        },
                        gas_price_estimator,
                    )
                    .await
                    .unwrap()],
                    cli::Mempool::Flashbots => {
                        join_all(args.flashbots_api_urls.iter().map(|url| async {
                            Mempool::flashbots(
                                mempool::Config {
                                    max_additional_tip: Some(
                                        args.submission.submission_max_additional_flashbots_tip,
                                    ),
                                    ..config.clone()
                                },
                                url.to_owned(),
                                gas_price_estimator.clone(),
                            )
                            .await
                            .unwrap()
                        }))
                        .await
                    }
                }
            }
        }))
        .await
        .into_iter()
        .flatten()
        .collect(),
        eth,
        addr: match args.bind_addr.as_str() {
            "auto" => api::Addr::Auto(addr_sender),
            addr => api::Addr::Bind(addr.parse().expect("a valid address and port")),
        },
        now,
        quote_config: competition::quote::Config {
            timeout: std::time::Duration::from_millis(args.quote_timeout_ms).into(),
        },
    }
    .serve(async {
        let _ = shutdown_receiver.await;
    });

    futures::pin_mut!(serve);
    tokio::select! {
        result = &mut serve => tracing::error!(?result, "API task exited"),
        _ = shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => tracing::error!("API shutdown exceeded timeout"),
            }
        }
    };
}

fn simulator(args: &cli::Args, eth: &Ethereum) -> Simulator {
    let simulator = if args.tenderly.is_specified() {
        Simulator::tenderly(simulator::tenderly::Config {
            url: args.tenderly.tenderly_url.clone(),
            api_key: args.tenderly.tenderly_api_key.clone().unwrap(),
            user: args.tenderly.tenderly_user.clone().unwrap(),
            project: args.tenderly.tenderly_project.clone().unwrap(),
            network_id: eth.network_id().to_owned(),
            save: args.tenderly.tenderly_save,
            save_if_fails: args.tenderly.tenderly_save_if_fails,
        })
    } else {
        Simulator::ethereum(eth.to_owned())
    };
    if args.disable_access_list_simulation {
        simulator.disable_access_lists()
    } else {
        simulator
    }
}

async fn ethereum(args: &cli::Args) -> Ethereum {
    Ethereum::ethrpc(
        &args.ethrpc,
        blockchain::contracts::Addresses {
            settlement: args.contract_addresses.gp_v2_settlement.map(Into::into),
            weth: args.contract_addresses.weth.map(Into::into),
        },
    )
    .await
    .expect("initialize ethereum RPC API")
}

async fn solvers(args: &cli::Args, now: infra::time::Now) -> Vec<Solver> {
    config::solvers::load(&args.solvers_config, now)
        .await
        .into_iter()
        .map(Solver::new)
        .collect()
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
