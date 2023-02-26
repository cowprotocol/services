use {
    crate::{
        helpers,
        onchain_components::{to_wei, OnchainComponents},
        services::{solvable_orders, wait_for_condition, API_HOST},
    },
    ethcontract::prelude::U256,
    model::{
        order::{Order, OrderBuilder, OrderClass, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    std::time::Duration,
    web3::signing::SecretKeyRef,
};

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
    helpers::init().await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(10)).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        token_b.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_router.add_liquidity(
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
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(10))
    );

    // Place Orders
    crate::services::start_autopilot(onchain.contracts(), &[]);
    crate::services::start_api(onchain.contracts(), &[]);
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
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
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
    crate::services::start_old_driver(onchain.contracts(), solver.private_key(), &[]);
    wait_for_condition(Duration::from_secs(10), || async {
        solvable_orders().await.unwrap() == 0
    })
    .await
    .unwrap();

    let balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(5));
}

async fn two_limit_orders_test(web3: Web3) {
    helpers::init().await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts and prepare funding Uniswap pool
    token_a.mint(trader_a.address(), to_wei(10)).await;
    token_b.mint(trader_b.address(), to_wei(10)).await;
    token_a.mint(solver.address(), to_wei(1_000)).await;
    token_b.mint(solver.address(), to_wei(1_000)).await;

    // Create and fund Uniswap pool
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        token_b.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_router.add_liquidity(
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
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(10))
    );
    tx!(
        trader_b.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(10))
    );

    // Place Orders
    crate::services::start_autopilot(onchain.contracts(), &[]);
    crate::services::start_api(onchain.contracts(), &[]);
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
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
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
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
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
    crate::services::start_old_driver(onchain.contracts(), solver.private_key(), &[]);
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
    helpers::init().await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(10)).await;
    token_b.mint(trader_b.address(), to_wei(6)).await;
    token_a.mint(solver.address(), to_wei(1000)).await;
    token_b.mint(solver.address(), to_wei(1000)).await;

    // Create and fund Uniswap pool
    tx!(
        solver.account(),
        onchain
            .contracts()
            .uniswap_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        token_b.approve(onchain.contracts().uniswap_router.address(), to_wei(1000))
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_router.add_liquidity(
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
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(10))
    );
    tx!(
        trader_b.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(6))
    );

    // Place Orders
    crate::services::start_autopilot(onchain.contracts(), &[]);
    crate::services::start_api(onchain.contracts(), &[]);
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
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
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
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
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
    crate::services::start_old_driver(onchain.contracts(), solver.private_key(), &[]);
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
    helpers::init().await;

    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_pools(to_wei(1_000), to_wei(1_000))
        .await;
    token_a.mint(trader.address(), to_wei(1)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(101))
    );

    // Place Orders
    crate::services::start_api(
        onchain.contracts(),
        &["--max-limit-orders-per-user=1".to_string()],
    );
    crate::services::wait_for_api_to_come_up().await;

    let client = reqwest::Client::default();

    let order = OrderBuilder::default()
        .with_sell_token(token_a.address())
        .with_sell_amount(to_wei(1))
        .with_buy_token(onchain.contracts().weth.address())
        .with_buy_amount(to_wei(1))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
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
        .with_buy_token(onchain.contracts().weth.address())
        .with_buy_amount(to_wei(2))
        .with_valid_to(model::time::now_in_epoch_seconds() + 300)
        .with_kind(OrderKind::Sell)
        .sign_with(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
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
