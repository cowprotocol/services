use {
    contracts::ERC20,
    e2e::{nodes::forked_node::ForkedNodeApi, setup::*, tx},
    ethcontract::{prelude::U256, H160},
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        signature::EcdsaSigningScheme,
    },
    secp256k1::SecretKey,
    shared::ethrpc::Web3,
    web3::signing::SecretKeyRef,
};

#[tokio::test]
#[ignore]
async fn local_node_single_limit_order() {
    run_test(single_limit_order_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_two_limit_orders() {
    run_test(two_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_too_many_limit_orders() {
    run_test(too_many_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_mixed_limit_and_market_orders() {
    run_test(mixed_limit_and_market_orders_test).await;
}

/// The block number from which we will fetch state for the forked tests.
pub const FORK_BLOCK: u64 = 18477910;
/// USDC whale address as per [FORK_BLOCK].
pub const USDC_WHALE: H160 = H160(hex_literal::hex!(
    "28c6c06298d514db089934071355e5743bf21d60"
));

#[tokio::test]
#[ignore]
async fn forked_node_single_limit_order_mainnet() {
    run_forked_test_with_block_number(
        forked_single_limit_order_test,
        std::env::var("FORK_URL").expect("FORK_URL must be set to run forked tests"),
        FORK_BLOCK,
    )
    .await;
}

async fn single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
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
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
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
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(5));
}

async fn two_limit_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
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
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
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
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order_a).await.unwrap();

    let limit_order = services.get_order(&order_id).await.unwrap();
    assert!(limit_order.metadata.class.is_limit());

    let order_b = OrderCreation {
        sell_token: token_b.address(),
        sell_amount: to_wei(5),
        buy_token: token_a.address(),
        buy_amount: to_wei(2),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::EthSign,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order_b).await.unwrap();

    let limit_order = services.get_order(&order_id).await.unwrap();
    assert!(limit_order.metadata.class.is_limit());

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 2 })
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_before_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 2 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let balance_after_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_after_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert!(balance_after_a.checked_sub(balance_before_a).unwrap() >= to_wei(5));
    assert!(balance_after_b.checked_sub(balance_before_b).unwrap() >= to_wei(2));
}

async fn mixed_limit_and_market_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
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
            .uniswap_v2_factory
            .create_pair(token_a.address(), token_b.address())
    );
    tx!(
        solver.account(),
        token_a.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        token_b.approve(
            onchain.contracts().uniswap_v2_router.address(),
            to_wei(1000)
        )
    );
    tx!(
        solver.account(),
        onchain.contracts().uniswap_v2_router.add_liquidity(
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
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_b.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order_a).await.unwrap();

    let limit_order = services.get_order(&order_id).await.unwrap();
    assert!(limit_order.metadata.class.is_limit());

    let order_b = OrderCreation {
        sell_token: token_b.address(),
        sell_amount: to_wei(5),
        fee_amount: to_wei(1),
        buy_token: token_a.address(),
        buy_amount: to_wei(2),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::EthSign,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order_b).await.unwrap();

    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Market);

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 2 })
        .await
        .unwrap();

    // Drive solution
    tracing::info!("Waiting for trade.");
    let balance_before_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_before_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 2 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let balance_after_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_after_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
    assert!(balance_after_a.checked_sub(balance_before_a).unwrap() >= to_wei(5));
    assert!(balance_after_b.checked_sub(balance_before_b).unwrap() >= to_wei(2));
}

async fn too_many_limit_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;
    token_a.mint(trader.address(), to_wei(1)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(101))
    );

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    services
        .start_api(vec!["--max-limit-orders-per-user=1".into()])
        .await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(1),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    services.create_order(&order).await.unwrap();

    // Attempt to place another order, but the orderbook is configured to allow only
    // one limit order per user.
    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(1),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(2),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let (status, body) = services.create_order(&order).await.unwrap_err();
    assert_eq!(status, 400);
    assert!(body.contains("TooManyLimitOrders"));
}

async fn forked_single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap(),
    );

    let token_usdt = ERC20::at(
        &web3,
        "0xdac17f958d2ee523a2206206994597c13d831ec7"
            .parse()
            .unwrap(),
    );

    // Give trader some USDC
    let usdc_whale = forked_node_api.impersonate(&USDC_WHALE).await.unwrap();
    tx!(
        usdc_whale,
        token_usdc.transfer(trader.address(), to_wei_with_exp(1000, 6))
    );

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token_usdc.approve(onchain.contracts().allowance, to_wei_with_exp(1000, 6))
    );

    // Place Orders
    let services = Services::new(onchain.contracts()).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(1000, 6),
        buy_token: token_usdt.address(),
        buy_amount: to_wei_with_exp(500, 6),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    let sell_token_balance_before = token_usdc
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    let buy_token_balance_before = token_usdt
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 1 })
        .await
        .unwrap();

    wait_for_condition(TIMEOUT, || async { services.solvable_orders().await == 0 })
        .await
        .unwrap();

    let sell_token_balance_after = token_usdc
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();
    let buy_token_balance_after = token_usdt
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    assert!(sell_token_balance_before > sell_token_balance_after);
    assert!(buy_token_balance_after >= buy_token_balance_before + to_wei_with_exp(500, 6));
}
