// TODO Remove dead_code ASAP
#![allow(dead_code)]
#![forbid(unsafe_code)]

mod boundary;
mod domain;
mod infra;
mod util;

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
    // `tokio::main` can have a bad effect on the IDE experience, hence this
    // workaround.
    run().await
}

async fn run() {
    let args = cli::Args::parse();
    shared::tracing::initialize("debug", LevelFilter::ERROR);
    shared::exit_process_on_panic::set_panic_hook();
    tracing::info!("running driver with validated arguments:\n{}", args);

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let eth = ethereum(&args).await;
    let serve = Api {
        solvers: solvers(&args).await,
        simulator: simulator(&args, &eth),
        eth,
        addr: args.bind_addr,
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
    if args.tenderly.is_specified() {
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
    }
}

async fn ethereum(args: &cli::Args) -> Ethereum {
    Ethereum::ethrpc(&args.ethrpc)
        .await
        .expect("initialize ethereum RPC API")
}

async fn solvers(args: &cli::Args) -> Vec<Solver> {
    config::solvers::load(&args.solvers_config)
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
