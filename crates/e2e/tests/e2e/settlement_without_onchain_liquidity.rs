use crate::{
    onchain_components::{
        deploy_token_with_weth_uniswap_pool, to_wei, uniswap_pair_provider, WethPoolConfig,
    },
    services::{
        create_order_converter, create_orderbook_api, wait_for_solvable_orders, OrderbookServices,
        API_HOST,
    },
};
use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use hex_literal::hex;
use model::{
    order::{OrderBuilder, OrderKind},
    signature::EcdsaSigningScheme,
};
use secp256k1::SecretKey;
use shared::{
    code_fetching::MockCodeFetching,
    ethrpc::Web3,
    http_client::HttpClientFactory,
    maintenance::Maintaining,
    sources::uniswap_v2::pool_fetching::PoolFetcher,
    token_list::{AutoUpdatingTokenList, Token},
};
use solver::{
    liquidity::uniswap_v2::UniswapLikeLiquidity,
    liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics,
    settlement_access_list::{create_priority_estimator, AccessListEstimatorType},
    settlement_post_processing::PostProcessingPipeline,
    settlement_submission::{
        submitter::{public_mempool_api::PublicMempoolApi, Strategy},
        GlobalTxPool, SolutionSubmitter, StrategyArgs,
    },
    solver::optimizing_solver::OptimizingSolver,
};
use std::{sync::Arc, time::Duration};
use web3::signing::SecretKeyRef;

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_onchain_settlement_without_liquidity() {
    crate::local_node::test(onchain_settlement_without_liquidity).await;
}

async fn onchain_settlement_without_liquidity(web3: Web3) {
    shared::tracing::initialize_reentrant("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_account = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);

    // Create & mint tokens to trade, pools for fee connections
    let token_a = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;
    let token_b = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(100_000),
            weth_amount: to_wei(100_000),
        },
    )
    .await;

    // Fund trader, settlement accounts, and pool creation
    token_a.mint(trader_account.address(), to_wei(100)).await;
    token_b
        .mint(contracts.gp_settlement.address(), to_wei(100))
        .await;
    token_a
        .mint(solver_account.address(), to_wei(100_000))
        .await;
    token_b
        .mint(solver_account.address(), to_wei(100_000))
        .await;
    let token_a = token_a.contract;
    let token_b = token_b.contract;

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver_account,
        token_a.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.approve(contracts.uniswap_router.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        contracts.uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(100_000),
            to_wei(100_000),
            0_u64.into(),
            0_u64.into(),
            solver_account.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(
        trader_account,
        token_a.approve(contracts.allowance, to_wei(100))
    );

    // Place Orders
    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, false).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(90))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_A_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order)
        .send()
        .await;
    assert_eq!(placement.unwrap().status(), 201);

    wait_for_solvable_orders(&client, 1).await.unwrap();

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        web3.clone(),
        Arc::new(PoolFetcher::uniswap(uniswap_pair_provider, web3.clone())),
    );

    let liquidity_collector = LiquidityCollector {
        liquidity_sources: vec![Box::new(uniswap_liquidity)],
        base_tokens,
    };
    let network_id = web3.net().version().await.unwrap();
    let market_makable_token_list = AutoUpdatingTokenList::new(maplit::hashmap! {
        token_a.address() => Token {
            address: token_a.address(),
            name: "Test Coin".into(),
            symbol: "TC".into(),
            decimals: 18,
        }
    });
    let post_processing_pipeline = Arc::new(PostProcessingPipeline::new(
        contracts.weth.address(),
        web3.clone(),
        1.,
        contracts.gp_settlement.clone(),
        market_makable_token_list,
    ));
    let solver = Arc::new(OptimizingSolver {
        inner: solver::solver::naive_solver(solver_account),
        post_processing_pipeline,
    });

    let submitted_transactions = GlobalTxPool::default();
    let mut driver = solver::driver::Driver::new(
        contracts.gp_settlement.clone(),
        liquidity_collector,
        vec![solver],
        Arc::new(web3.clone()),
        Duration::from_secs(30),
        contracts.weth.address(),
        Duration::from_secs(0),
        Arc::new(NoopMetrics::default()),
        web3.clone(),
        network_id.clone(),
        Duration::from_secs(10),
        block_stream,
        SolutionSubmitter {
            web3: web3.clone(),
            contract: contracts.gp_settlement.clone(),
            gas_price_estimator: Arc::new(web3.clone()),
            target_confirm_time: Duration::from_secs(1),
            gas_price_cap: f64::MAX,
            max_confirm_time: Duration::from_secs(120),
            retry_interval: Duration::from_secs(5),
            transaction_strategies: vec![
                solver::settlement_submission::TransactionStrategy::PublicMempool(StrategyArgs {
                    submit_api: Box::new(PublicMempoolApi::new(vec![web3.clone()], false)),
                    max_additional_tip: 0.,
                    additional_tip_percentage_of_max_fee: 0.,
                    sub_tx_pool: submitted_transactions.add_sub_pool(Strategy::PublicMempool),
                }),
            ],
            access_list_estimator: Arc::new(
                create_priority_estimator(
                    &web3,
                    &[AccessListEstimatorType::Web3],
                    None,
                    network_id,
                )
                .unwrap(),
            ),
            code_fetcher: Arc::new(MockCodeFetching::new()),
        },
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        15000000u128,
        1.0,
        None,
        None.into(),
        None,
        0,
        Arc::new(MockCodeFetching::new()),
    );
    driver.single_run().await.unwrap();

    // Check that trader traded.
    let balance = token_a
        .balance_of(trader_account.address())
        .call()
        .await
        .expect("Couldn't fetch trader TokenA's balance");
    assert_eq!(balance, U256::from(0_u128));

    let balance = token_b
        .balance_of(trader_account.address())
        .call()
        .await
        .expect("Couldn't fetch trader TokenB's balance");
    assert!(balance > U256::zero());

    // Check that settlement buffers were traded.
    let balance = token_a
        .balance_of(contracts.gp_settlement.address())
        .call()
        .await
        .expect("Couldn't fetch settlements TokenA's balance");
    assert_eq!(balance, to_wei(100));

    // Drive orderbook in order to check the removal of settled order_b
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    let auction = create_orderbook_api().get_auction().await.unwrap();
    assert!(auction.auction.orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}
