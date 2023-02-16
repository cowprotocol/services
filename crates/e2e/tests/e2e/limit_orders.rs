use {
    crate::{
        onchain_components::{deploy_token_with_weth_uniswap_pool, to_wei, WethPoolConfig},
        services::{solvable_orders, wait_for_condition, API_HOST},
    },
    ethcontract::{
        prelude::{Account, PrivateKey, U256},
        transaction::TransactionBuilder,
    },
    hex_literal::hex,
    model::{
        order::{Order, OrderBuilder, OrderClass, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

const TRADER_A_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000001");
const TRADER_B_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000002");
const SOLVER_PK: [u8; 32] =
    hex!("0000000000000000000000000000000000000000000000000000000000000003");

const ORDER_PLACEMENT_ENDPOINT: &str = "/api/v1/orders/";

#[tokio::test]
#[ignore]
async fn local_node_single_limit_order() {
    crate::local_node::test(single_limit_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_two_limit_orders() {
    crate::local_node::test(two_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_too_many_limit_orders() {
    crate::local_node::test(too_many_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_mixed_limit_and_market_orders() {
    crate::local_node::test(mixed_limit_and_market_orders_test).await;
}

async fn single_limit_order_test(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PK).unwrap(), None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    for account in [&trader_a, &solver] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(1))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();

    // Create & mint tokens to trade, pools for fee connections
    let token_a = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    let token_b = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(10)).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    let token_a = token_a.contract;
    let token_b = token_b.contract;
    tx!(
        solver,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver,
        token_a.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        token_b.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        contracts.uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(contracts.allowance, to_wei(10)));

    // Place Orders
    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(10))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(5))
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
    assert_eq!(
        limit_order.metadata.class,
        OrderClass::Limit(Default::default())
    );

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 1
    })
    .await
    .unwrap();
    crate::services::start_old_driver(&contracts, &SOLVER_PK, &[]);
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();

    let balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(5));
}

async fn two_limit_orders_test(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PK).unwrap(), None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);
    for account in [&solver, &trader_a, &trader_b] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(1))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();

    // Create & mint tokens to trade, pools for fee connections
    let token_a = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    let token_b = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;

    // Fund trader accounts and prepare funding Uniswap pool
    token_a.mint(trader_a.address(), to_wei(10)).await;
    token_b.mint(trader_b.address(), to_wei(10)).await;
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    let token_a = token_a.contract;
    let token_b = token_b.contract;

    // Create and fund Uniswap pool
    tx!(
        solver,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver,
        token_a.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        token_b.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        contracts.uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(contracts.allowance, to_wei(10)));
    tx!(trader_b, token_b.approve(contracts.allowance, to_wei(10)));

    // Place Orders
    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(10))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(5))
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
    assert!(limit_order.metadata.class.is_limit());

    let order_b = OrderBuilder::default()
        .with_sell_token(token_b.address())
        .with_sell_amount(to_wei(5))
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(2))
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
    assert!(limit_order.metadata.class.is_limit());

    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_before_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();
    crate::services::start_old_driver(&contracts, &SOLVER_PK, &[]);
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();

    let balance_after_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_after_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert!(balance_after_a.checked_sub(balance_before_a).unwrap() >= to_wei(5));
    assert!(balance_after_b.checked_sub(balance_before_b).unwrap() >= to_wei(2));
}

async fn mixed_limit_and_market_orders_test(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let solver = Account::Offline(PrivateKey::from_raw(SOLVER_PK).unwrap(), None);
    let trader_a = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    let trader_b = Account::Offline(PrivateKey::from_raw(TRADER_B_PK).unwrap(), None);
    for account in [&solver, &trader_a, &trader_b] {
        TransactionBuilder::new(web3.clone())
            .value(to_wei(1))
            .to(account.address())
            .send()
            .await
            .unwrap();
    }

    contracts
        .gp_authenticator
        .add_solver(solver.address())
        .send()
        .await
        .unwrap();

    // Create & mint tokens to trade, pools for fee connections
    let token_a = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    let token_b = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(10)).await;
    token_b.mint(trader_b.address(), to_wei(6)).await;
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    let token_a = token_a.contract;
    let token_b = token_b.contract;

    // Create and fund Uniswap pool
    tx!(
        solver,
        contracts
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver,
        token_a.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        token_b.approve(contracts.uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver,
        contracts.uniswap_router.add_liquidity(
            token_a.address(),
            token_b.address(),
            to_wei(1000),
            to_wei(1000),
            0_u64.into(),
            0_u64.into(),
            solver.address(),
            U256::max_value(),
        )
    );

    // Approve GPv2 for trading
    tx!(trader_a, token_a.approve(contracts.allowance, to_wei(10)));
    tx!(trader_b, token_b.approve(contracts.allowance, to_wei(6)));

    // Place Orders
    crate::services::start_autopilot(&contracts, &[]);
    crate::services::start_api(&contracts, &[]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order_a = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(10))
        .with_buy_token(token_b.address())
        .with_buy_amount(to_wei(5))
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
    assert!(limit_order.metadata.class.is_limit());

    let order_b = OrderBuilder::default()
        .with_sell_token(token_b.address())
        .with_sell_amount(to_wei(5))
        .with_fee_amount(to_wei(1))
        .with_buy_token(token_a.address())
        .with_buy_amount(to_wei(2))
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

    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_before_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 2
    })
    .await
    .unwrap();
    crate::services::start_old_driver(&contracts, &SOLVER_PK, &[]);
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();

    let balance_after_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_after_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert!(balance_after_a.checked_sub(balance_before_a).unwrap() >= to_wei(5));
    assert!(balance_after_b.checked_sub(balance_before_b).unwrap() >= to_wei(2));
}

async fn too_many_limit_orders_test(web3: Web3) {
    shared::tracing::initialize_reentrant(
        "e2e=debug,orderbook=debug,solver=debug,autopilot=debug,\
         orderbook::api::request_summary=off",
    );
    shared::exit_process_on_panic::set_panic_hook();

    crate::services::clear_database().await;
    let contracts = crate::deploy::deploy(&web3).await.expect("deploy");

    let trader = Account::Offline(PrivateKey::from_raw(TRADER_A_PK).unwrap(), None);
    TransactionBuilder::new(web3.clone())
        .value(to_wei(1))
        .to(trader.address())
        .send()
        .await
        .unwrap();

    // Create & Mint tokens to trade
    let token_a = deploy_token_with_weth_uniswap_pool(
        &web3,
        &contracts,
        WethPoolConfig {
            token_amount: to_wei(1000),
            weth_amount: to_wei(1000),
        },
    )
    .await;
    token_a.mint(trader.address(), to_wei(1)).await;
    let token_a = token_a.contract;

    // Approve GPv2 for trading
    tx!(trader, token_a.approve(contracts.allowance, to_wei(101)));

    // Place Orders
    crate::services::start_api(&contracts, &["--max-limit-orders-per-user=1".to_string()]);
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(1))
        .with_buy_token(contracts.weth.address())
        .with_buy_amount(to_wei(1))
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

    // Attempt to place another order, but the orderbook is configured to allow only
    // one limit order per user.
    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(1))
        .with_buy_token(contracts.weth.address())
        .with_buy_amount(to_wei(2))
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
