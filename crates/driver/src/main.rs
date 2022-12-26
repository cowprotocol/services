// TODO Remove dead_code ASAP
#![allow(dead_code)]
#![forbid(unsafe_code)]

mod api;
mod blockchain;
pub mod boundary;
pub mod cli;
pub mod logic;
pub mod simulator;
pub mod solver;
mod util;

pub use {crate::solver::Solver, blockchain::Ethereum, simulator::Simulator};
use {
    api::Api,
    clap::Parser,
    logic::eth,
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
        solvers: vec![Solver::new(solver::Config {
            url: "http://localhost:1232".parse().unwrap(),
            name: "solver".to_owned().into(),
            account: solver_account(),
            address: solver_address(),
            slippage: solver::Slippage {
                // TODO These should be fetched from the configuration
                relative: Default::default(),
                absolute: Default::default(),
            },
        })],
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
            url: args
                .tenderly
                .tenderly_url
                .as_ref()
                .map(|url| url.to_string().parse().unwrap()),
            api_key: args.tenderly.tenderly_api_key.clone().unwrap(),
            user: args.tenderly.tenderly_user.clone().unwrap(),
            project: args.tenderly.tenderly_project.clone().unwrap(),
            network_id: eth.network_id().to_owned(),
            // TODO These should also be CLI args
            save: true,
            save_if_fails: true,
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

// TODO For solvers, I feel like we should have a YAML or JSON file and only
// specify a path to it, otherwise we get into nightmare land. Opinions?

fn solver_account() -> eth::Account {
    todo!()
}

fn solver_address() -> eth::Address {
    todo!()
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept signals for graceful shutdown. Kubernetes sends sigterm, Ctrl-C
    // sends sigint (ctrl-c).
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
