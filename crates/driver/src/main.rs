use clap::Parser;
use contracts::WETH9;
use driver::{
    api::serve_api, arguments::Arguments, commit_reveal::CommitRevealSolver, driver::Driver,
};
use reqwest::Client;
use shared::{
    http_solver::{DefaultHttpSolverApi, SolverConfig},
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    transport::{create_instrumented_transport, http::HttpTransport},
};
use solver::{
    interactions::allowances::AllowanceManager,
    metrics::Metrics,
    solver::{
        http_solver::{buffers::BufferRetriever, HttpSolver, InstanceCache},
        Solver,
    },
};
use std::{sync::Arc, time::Duration};

struct CommonComponents {
    client: Client,
    web3: shared::Web3,
    network_id: String,
    chain_id: u64,
    settlement_contract: contracts::GPv2Settlement,
    native_token_contract: WETH9,
}

async fn init_common_components(args: &Arguments) -> CommonComponents {
    let client = shared::http_client(args.http_timeout);
    let metrics = Arc::new(Metrics::new().expect("Couldn't register metrics"));
    let transport = create_instrumented_transport(
        HttpTransport::new(client.clone(), args.node_url.clone(), "base".to_string()),
        metrics.clone(),
    );
    let web3 = web3::Web3::new(transport);
    let network_id = web3
        .net()
        .version()
        .await
        .expect("failed to get network id");
    let chain_id = web3
        .eth()
        .chain_id()
        .await
        .expect("Could not get chainId")
        .as_u64();
    let settlement_contract = solver::get_settlement_contract(&web3)
        .await
        .expect("couldn't load deployed settlement");
    let native_token_contract = WETH9::deployed(&web3)
        .await
        .expect("couldn't load deployed native token");

    CommonComponents {
        client,
        web3,
        network_id,
        chain_id,
        settlement_contract,
        native_token_contract,
    }
}

async fn build_solvers(common: &CommonComponents, args: &Arguments) -> Vec<Box<dyn Solver>> {
    let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
        web3: common.web3.clone(),
    })));

    let buffer_retriever = Arc::new(BufferRetriever::new(
        common.web3.clone(),
        common.settlement_contract.address(),
    ));
    let allowance_mananger = Arc::new(AllowanceManager::new(
        common.web3.clone(),
        common.settlement_contract.address(),
    ));
    let http_solver_cache = InstanceCache::default();

    args.solvers
        .iter()
        .map(|arg| {
            Box::new(HttpSolver::new(
                DefaultHttpSolverApi {
                    name: arg.name.clone(),
                    network_name: common.network_id.clone(),
                    chain_id: common.chain_id,
                    base: arg.url.clone(),
                    client: common.client.clone(),
                    config: SolverConfig {
                        use_internal_buffers: Some(args.use_internal_buffers),
                        ..Default::default()
                    },
                },
                arg.account.clone().into_account(common.chain_id),
                common.native_token_contract.address(),
                token_info_fetcher.clone(),
                buffer_retriever.clone(),
                allowance_mananger.clone(),
                http_solver_cache.clone(),
            )) as Box<dyn Solver>
        })
        .collect()
}

#[tokio::main]
async fn main() {
    let args = driver::arguments::Arguments::parse();
    shared::tracing::initialize(args.log_filter.as_str(), args.log_stderr_threshold);
    tracing::info!("running driver with validated arguments:\n{}", args);
    global_metrics::setup_metrics_registry(Some("gp_v2_driver".into()), None);
    let common = init_common_components(&args).await;
    let solvers = build_solvers(&common, &args).await;

    let drivers = solvers
        .into_iter()
        .map(|solver| {
            let name = solver.name().to_string();
            let driver = Arc::new(Driver {
                solver: Arc::new(CommitRevealSolver::new(solver)),
            });
            (driver, name)
        })
        .collect();

    let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
    let serve_api = serve_api(
        args.bind_address,
        async {
            let _ = shutdown_receiver.await;
        },
        drivers,
    );

    futures::pin_mut!(serve_api);
    tokio::select! {
        result = &mut serve_api => tracing::error!(?result, "API task exited"),
        _ = shutdown_signal() => {
            tracing::info!("Gracefully shutting down API");
            shutdown_sender.send(()).expect("failed to send shutdown signal");
            match tokio::time::timeout(Duration::from_secs(10), serve_api).await {
                Ok(inner) => inner.expect("API failed during shutdown"),
                Err(_) => tracing::error!("API shutdown exceeded timeout"),
            }
        }
    };
}

#[cfg(unix)]
async fn shutdown_signal() {
    // Intercept main signals for graceful shutdown
    // Kubernetes sends sigterm, whereas locally sigint (ctrl-c) is most common
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
    // We don't support signal handling on windows
    std::future::pending().await
}
