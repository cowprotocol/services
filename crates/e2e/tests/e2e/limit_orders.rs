use std::sync::Arc;

use crate::services::{
    create_order_converter, create_orderbook_api, deploy_mintable_token, to_wei,
    uniswap_pair_provider, wait_for_solvable_orders, OrderbookServices, API_HOST,
};
use contracts::IUniswapLikeRouter;
use ethcontract::prelude::{Account, Address, PrivateKey, U256};
use hex_literal::hex;
use model::{
    order::{Order, OrderBuilder, OrderClass, OrderKind},
    signature::EcdsaSigningScheme,
};
use secp256k1::SecretKey;
use shared::{
    ethrpc::Web3, http_client::HttpClientFactory, maintenance::Maintaining,
    sources::uniswap_v2::pool_fetching::PoolFetcher,
};
use solver::{
    liquidity::uniswap_v2::UniswapLikeLiquidity,
    liquidity_collector::LiquidityCollector,
    metrics::NoopMetrics,
    settlement_access_list::{create_priority_estimator, AccessListEstimatorType},
    settlement_submission::{
        submitter::{public_mempool_api::PublicMempoolApi, Strategy},
        GlobalTxPool, SolutionSubmitter, StrategyArgs,
    },
};
use std::time::Duration;
use web3::signing::SecretKeyRef;

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const TRADER_B_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000002");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn single_limit_order() {
    crate::local_node::test(single_limit_order_test).await;
}

#[tokio::test]
#[ignore]
async fn two_limit_orders() {
    crate::local_node::test(two_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn too_many_limit_orders() {
    crate::local_node::test(too_many_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn mixed_limit_and_market_orders() {
    crate::local_node::test(mixed_limit_and_market_orders_test).await;
}

async fn single_limit_order_test(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);

    // Create tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    let token_b = deploy_mintable_token(&web3).await;

    // Fund trader accounts
    tx!(
        solver_account,
        token_a.mint(trader_a.address(), to_wei(1010))
    );
    tx!(
        solver_account,
        token_b.mint(trader_b.address(), to_wei(510))
    );

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver_account,
        token_a.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.mint(solver_account.address(), to_wei(100_000))
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

    // Create and fund pools for fee connections.
    for token in [&token_a, &token_b] {
        tx!(
            solver_account,
            token.mint(solver_account.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            token.approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx_value!(solver_account, to_wei(100_000), contracts.weth.deposit());
        tx!(
            solver_account,
            contracts
                .weth
                .approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            contracts.uniswap_router.add_liquidity(
                token.address(),
                contracts.weth.address(),
                to_wei(100_000),
                to_wei(100_000),
                0_u64.into(),
                0_u64.into(),
                solver_account.address(),
                U256::max_value(),
            )
        );
    }

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(contracts.allowance, to_wei(101)));
    tx!(trader_b, token_b.approve(contracts.allowance, to_wei(51)));

    // Place Orders
    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, true).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
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
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);
    let order_id: String = placement.json().await.unwrap();

    let limit_order: Order = client
        .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_id}"))
        .json(&order)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    wait_for_solvable_orders(&client, 1).await.unwrap();

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        base_tokens,
        web3.clone(),
        Arc::new(PoolFetcher::uniswap(uniswap_pair_provider, web3.clone())),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        balancer_v2_liquidity: None,
        zeroex_liquidity: None,
        uniswap_v3_liquidity: None,
    };
    let network_id = web3.net().version().await.unwrap();
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
        Duration::from_secs(30),
        Default::default(),
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
        },
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        0.0,
        15000000u128,
        1.0,
        None,
        None.into(),
        None,
        0,
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .expect("Couldn't fetch TokenB's balance");
    assert_eq!(balance, U256::from(99_600_698_103_990_321_648_u128));

    let balance = token_a
        .balance_of(trader_b.address())
        .call()
        .await
        .expect("Couldn't fetch TokenA's balance");
    // Didn't touch the balance of token_a
    assert_eq!(balance, U256::zero());

    // Drive orderbook in order to check the removal of settled order_b
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    let auction = create_orderbook_api().get_auction().await.unwrap();
    assert!(auction.auction.orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}

async fn two_limit_orders_test(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);

    // Create tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    let token_b = deploy_mintable_token(&web3).await;

    // Fund trader accounts
    tx!(
        solver_account,
        token_a.mint(trader_a.address(), to_wei(1010))
    );
    tx!(
        solver_account,
        token_b.mint(trader_b.address(), to_wei(510))
    );

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver_account,
        token_a.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.mint(solver_account.address(), to_wei(100_000))
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

    // Create and fund pools for fee connections.
    for token in [&token_a, &token_b] {
        tx!(
            solver_account,
            token.mint(solver_account.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            token.approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx_value!(solver_account, to_wei(100_000), contracts.weth.deposit());
        tx!(
            solver_account,
            contracts
                .weth
                .approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            contracts.uniswap_router.add_liquidity(
                token.address(),
                contracts.weth.address(),
                to_wei(100_000),
                to_wei(100_000),
                0_u64.into(),
                0_u64.into(),
                solver_account.address(),
                U256::max_value(),
            )
        );
    }

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(contracts.allowance, to_wei(101)));
    tx!(trader_b, token_b.approve(contracts.allowance, to_wei(51)));

    // Place Orders
    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, true).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
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
        .json(&order_a)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);
    let order_id: String = placement.json().await.unwrap();

    let limit_order: Order = client
        .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_id}"))
        .json(&order_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    let order_b = OrderBuilder::default()
        .with_sell_token(token_b.address())
        .with_sell_amount(to_wei(50))
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(40))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::EthSign,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_B_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_b)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);
    let order_id: String = placement.json().await.unwrap();

    let limit_order: Order = client
        .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_id}"))
        .json(&order_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    wait_for_solvable_orders(&client, 2).await.unwrap();

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        base_tokens,
        web3.clone(),
        Arc::new(PoolFetcher::uniswap(uniswap_pair_provider, web3.clone())),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        balancer_v2_liquidity: None,
        zeroex_liquidity: None,
        uniswap_v3_liquidity: None,
    };
    let network_id = web3.net().version().await.unwrap();
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
        Duration::from_secs(30),
        Default::default(),
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
        },
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        0.0,
        15000000u128,
        1.0,
        None,
        None.into(),
        None,
        0,
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .expect("Couldn't fetch TokenB's balance");
    assert_eq!(balance, U256::from(99_650_498_453_042_316_811_u128));

    let balance = token_a
        .balance_of(trader_b.address())
        .call()
        .await
        .expect("Couldn't fetch TokenA's balance");
    assert_eq!(balance, U256::from(50_175_363_672_226_073_522_u128));

    // Drive orderbook in order to check the removal of settled order_b
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    let auction = create_orderbook_api().get_auction().await.unwrap();
    assert!(auction.auction.orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}

async fn mixed_limit_and_market_orders_test(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);

    // Create tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    let token_b = deploy_mintable_token(&web3).await;

    // Fund trader accounts
    tx!(
        solver_account,
        token_a.mint(trader_a.address(), to_wei(1010))
    );
    tx!(
        solver_account,
        token_b.mint(trader_b.address(), to_wei(510))
    );

    // Create and fund Uniswap pool
    tx!(
        solver_account,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver_account,
        token_a.mint(solver_account.address(), to_wei(100_000))
    );
    tx!(
        solver_account,
        token_b.mint(solver_account.address(), to_wei(100_000))
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

    // Fund settlement contract
    tx!(
        solver_account,
        token_a.mint(contracts.gp_settlement.address(), to_wei(1010))
    );

    // Create and fund pools for fee connections.
    for token in [&token_a, &token_b] {
        tx!(
            solver_account,
            token.mint(solver_account.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            token.approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx_value!(solver_account, to_wei(100_000), contracts.weth.deposit());
        tx!(
            solver_account,
            contracts
                .weth
                .approve(contracts.uniswap_router.address(), to_wei(100_000))
        );
        tx!(
            solver_account,
            contracts.uniswap_router.add_liquidity(
                token.address(),
                contracts.weth.address(),
                to_wei(100_000),
                to_wei(100_000),
                0_u64.into(),
                0_u64.into(),
                solver_account.address(),
                U256::max_value(),
            )
        );
    }

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(contracts.allowance, to_wei(101)));
    tx!(trader_b, token_b.approve(contracts.allowance, to_wei(51)));

    // Place Orders
    let OrderbookServices {
        maintenance,
        block_stream,
        solvable_orders_cache,
        base_tokens,
        ..
    } = OrderbookServices::new(&web3, &contracts, true).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
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
        .json(&order_a)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);
    let order_id: String = placement.json().await.unwrap();

    let limit_order: Order = client
        .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_id}"))
        .json(&order_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    let order_b = OrderBuilder::default()
        .with_sell_token(token_b.address())
        .with_sell_amount(to_wei(50))
        .with_fee_amount(1.into())
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(40))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::EthSign,
            &contracts.domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(&TRADER_B_PK).unwrap()),
        )
        .build()
        .into_order_creation();
    let placement = client
        .post(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}"))
        .json(&order_b)
        .send()
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);
    let order_id: String = placement.json().await.unwrap();

    let limit_order: Order = client
        .get(&format!("{API_HOST}{ORDER_PLACEMENT_ENDPOINT}{order_id}"))
        .json(&order_a)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Market);

    wait_for_solvable_orders(&client, 2).await.unwrap();

    // Drive solution
    let uniswap_pair_provider = uniswap_pair_provider(&contracts);
    let uniswap_liquidity = UniswapLikeLiquidity::new(
        IUniswapLikeRouter::at(&web3, contracts.uniswap_router.address()),
        contracts.gp_settlement.clone(),
        base_tokens,
        web3.clone(),
        Arc::new(PoolFetcher::uniswap(uniswap_pair_provider, web3.clone())),
    );
    let solver = solver::solver::naive_solver(solver_account);
    let liquidity_collector = LiquidityCollector {
        uniswap_like_liquidity: vec![uniswap_liquidity],
        balancer_v2_liquidity: None,
        zeroex_liquidity: None,
        uniswap_v3_liquidity: None,
    };
    let network_id = web3.net().version().await.unwrap();
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
        Duration::from_secs(30),
        Default::default(),
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
        },
        create_orderbook_api(),
        create_order_converter(&web3, contracts.weth.address()),
        0.0,
        15000000u128,
        1.0,
        None,
        None.into(),
        None,
        0,
    );
    driver.single_run().await.unwrap();

    // Check matching
    let balance = token_b
        .balance_of(trader_a.address())
        .call()
        .await
        .expect("Couldn't fetch TokenB's balance");
    assert_eq!(balance, U256::from(99_650_498_453_042_316_811_u128));

    let balance = token_a
        .balance_of(trader_b.address())
        .call()
        .await
        .expect("Couldn't fetch TokenA's balance");
    assert_eq!(balance, U256::from(50_175_363_672_226_073_523_u128));

    // Drive orderbook in order to check the removal of settled order_b
    maintenance.run_maintenance().await.unwrap();
    solvable_orders_cache.update(0).await.unwrap();

    let auction = create_orderbook_api().get_auction().await.unwrap();
    assert!(auction.auction.orders.is_empty());

    // Drive again to ensure we can continue solution finding
    driver.single_run().await.unwrap();
}

async fn too_many_limit_orders_test(web3: Web3) {
    shared::tracing::initialize_for_tests("warn,orderbook=debug,solver=debug,autopilot=debug");
    shared::exit_process_on_panic::set_panic_hook();
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let accounts: Vec<Address> = web3.eth().accounts().await.expect("get accounts failed");
    let solver_account = Account::Local(accounts[0], None);
    let trader_account = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);

    // Create & Mint tokens to trade
    let token_a = deploy_mintable_token(&web3).await;
    let token_b = deploy_mintable_token(&web3).await;

    // Fund trader and settlement accounts
    tx!(
        solver_account,
        token_a.mint(trader_account.address(), to_wei(100))
    );
    tx!(
        solver_account,
        token_b.mint(contracts.gp_settlement.address(), to_wei(100))
    );

    // Approve GPv2 for trading
    tx!(
        trader_account,
        token_a.approve(contracts.allowance, to_wei(100))
    );

    // Place Orders
    let _services = OrderbookServices::new(&web3, &contracts, true).await;

    let http_factory = HttpClientFactory::default();
    let client = http_factory.create();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(80))
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
        .await
        .unwrap();
    assert_eq!(placement.status(), 201);

    // Attempt to place another order, but the orderbook is configured to allow only one limit
    // order per user.
    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(100))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(1200))
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
        .await
        .unwrap();
    assert_eq!(placement.status(), 400);
    assert!(placement
        .text()
        .await
        .unwrap()
        .contains("TooManyLimitOrders"));
}
