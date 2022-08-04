use anyhow::Context;
use clap::Parser;
use contracts::WETH9;
use driver::{
    api::serve_api, arguments::Arguments, commit_reveal::CommitRevealSolver, driver::Driver,
};
use reqwest::Client;
use shared::{
    http_solver::{DefaultHttpSolverApi, SolverConfig},
    token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    transport::http::HttpTransport,
    Web3Transport,
};
use solver::{
    arguments::TransactionStrategyArg,
    interactions::allowances::AllowanceManager,
    settlement_submission::{
        submitter::{
            custom_nodes_api::CustomNodesApi, eden_api::EdenApi, flashbots_api::FlashbotsApi,
            Strategy,
        },
        GlobalTxPool, SolutionSubmitter, StrategyArgs, TransactionStrategy,
    },
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
    let transport = Web3Transport::new(HttpTransport::new(
        client.clone(),
        args.node_url.clone(),
        "base".to_string(),
    ));
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

async fn build_submitter(common: &CommonComponents, args: &Arguments) -> Arc<SolutionSubmitter> {
    let client = &common.client;
    let web3 = &common.web3;

    let submission_nodes_with_url = args
        .transaction_submission_nodes
        .iter()
        .enumerate()
        .map(|(index, url)| {
            let transport = Web3Transport::new(HttpTransport::new(
                client.clone(),
                url.clone(),
                index.to_string(),
            ));
            (web3::Web3::new(transport), url)
        })
        .collect::<Vec<_>>();
    for (node, url) in &submission_nodes_with_url {
        let node_network_id = node
            .net()
            .version()
            .await
            .with_context(|| {
                format!(
                    "Unable to retrieve network id on startup using the submission node at {url}"
                )
            })
            .unwrap();
        assert_eq!(
            node_network_id, common.network_id,
            "network id of custom node doesn't match main node"
        );
    }
    let submission_nodes = submission_nodes_with_url
        .into_iter()
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    let submitted_transactions = GlobalTxPool::default();
    let mut transaction_strategies = vec![];
    for strategy in &args.transaction_strategy {
        match strategy {
            TransactionStrategyArg::PublicMempool => {
                transaction_strategies.push(TransactionStrategy::CustomNodes(StrategyArgs {
                    submit_api: Box::new(CustomNodesApi::new(vec![web3.clone()])),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::CustomNodes),
                }))
            }
            TransactionStrategyArg::Eden => {
                transaction_strategies.push(TransactionStrategy::Eden(StrategyArgs {
                    submit_api: Box::new(
                        EdenApi::new(
                            client.clone(),
                            args.eden_api_url.clone(),
                            submitted_transactions.clone(),
                        )
                        .unwrap(),
                    ),
                    max_additional_tip: args.max_additional_eden_tip,
                    additional_tip_percentage_of_max_fee: args.additional_tip_percentage,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::Eden),
                }))
            }
            TransactionStrategyArg::Flashbots => {
                for flashbots_url in args.flashbots_api_url.clone() {
                    transaction_strategies.push(TransactionStrategy::Flashbots(StrategyArgs {
                        submit_api: Box::new(
                            FlashbotsApi::new(client.clone(), flashbots_url).unwrap(),
                        ),
                        max_additional_tip: args.max_additional_flashbot_tip,
                        additional_tip_percentage_of_max_fee: args.additional_tip_percentage,
                        sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::Flashbots),
                    }))
                }
            }
            TransactionStrategyArg::CustomNodes => {
                assert!(
                    !submission_nodes.is_empty(),
                    "missing transaction submission nodes"
                );
                transaction_strategies.push(TransactionStrategy::CustomNodes(StrategyArgs {
                    submit_api: Box::new(CustomNodesApi::new(submission_nodes.clone())),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::CustomNodes),
                }))
            }
            TransactionStrategyArg::DryRun => {
                transaction_strategies.push(TransactionStrategy::DryRun)
            }
        }
    }
    let access_list_estimator = Arc::new(
        solver::settlement_access_list::create_priority_estimator(
            client,
            web3,
            args.access_list_estimators.as_slice(),
            args.tenderly_url.clone(),
            args.tenderly_api_key.clone(),
            common.network_id.clone(),
        )
        .await
        .expect("failed to create access list estimator"),
    );
    let gas_price_estimator = Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            client.clone(),
            web3,
            args.gas_estimators.as_slice(),
            args.blocknative_api_key.clone(),
        )
        .await
        .expect("failed to create gas price estimator"),
    );

    Arc::new(SolutionSubmitter {
        web3: web3.clone(),
        contract: common.settlement_contract.clone(),
        gas_price_estimator,
        target_confirm_time: args.target_confirm_time,
        max_confirm_time: args.max_submission_seconds,
        retry_interval: args.submission_retry_interval_seconds,
        gas_price_cap: args.gas_price_cap,
        transaction_strategies,
        access_list_estimator,
    })
}

#[tokio::main]
async fn main() {
    let args = driver::arguments::Arguments::parse();
    shared::tracing::initialize(args.log_filter.as_str(), args.log_stderr_threshold);
    tracing::info!("running driver with validated arguments:\n{}", args);
    global_metrics::setup_metrics_registry(Some("gp_v2_driver".into()), None);
    let common = init_common_components(&args).await;
    let solvers = build_solvers(&common, &args).await;
    let submitter = build_submitter(&common, &args).await;

    let drivers = solvers
        .into_iter()
        .map(|solver| {
            let name = solver.name().to_string();
            let driver = Arc::new(Driver {
                solver: Arc::new(CommitRevealSolver::new(solver)),
                submitter: submitter.clone(),
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
