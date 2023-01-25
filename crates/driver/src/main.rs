// TODO Remove dead_code ASAP
#![allow(dead_code)]
#![forbid(unsafe_code)]

use {
    crate::{
        domain::competition,
        infra::{mempool, Mempool},
    },
    clap::Parser,
    config::cli,
    futures::future::join_all,
    infra::{
        blockchain::{self, Ethereum},
        config,
        liquidity,
        simulator::{self, Simulator},
        solver::{self, Solver},
        Api,
    },
    std::{net::SocketAddr, time::Duration},
    tokio::sync::oneshot,
    tracing::level_filters::LevelFilter,
};

mod boundary;
mod domain;
mod infra;
mod util;

#[cfg(test)]
mod tests;

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

    let config = config::file::load(&args.config).await;

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let eth = ethereum(&config, &args).await;
    let tx_pool = mempool::GlobalTxPool::default();
    let serve = Api {
        solvers: solvers(&config, now),
        liquidity: liquidity(&config, &eth).await,
        simulator: simulator(&config, &eth),
        mempools: join_all(
            config
                .mempools
                .iter()
                .map(|mempool| Mempool::new(mempool.to_owned(), eth.clone(), tx_pool.clone())),
        )
        .await
        .into_iter()
        .flatten()
        .collect(),
        eth,
        now,
        quote_config: competition::quote::Config {
            // TODO Nick is removing this in one of his PRs
            timeout: std::time::Duration::from_millis(100).into(),
        },
        addr: args.bind_addr,
        addr_sender,
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

fn simulator(config: &infra::Config, eth: &Ethereum) -> Simulator {
    let simulator = match &config.tenderly {
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
    } else {
        simulator
    }
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

fn solvers(config: &config::Config, now: infra::time::Now) -> Vec<Solver> {
    config
        .solvers
        .iter()
        .map(|config| Solver::new(config.clone(), now))
        .collect()
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
