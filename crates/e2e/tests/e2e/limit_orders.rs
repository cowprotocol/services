use {
    contracts::ERC20,
    e2e::{nodes::forked_node::ForkedNodeApi, setup::*, tx},
    ethcontract::{prelude::U256, H160},
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
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
const FORK_BLOCK_MAINNET: u64 = 18477910;
/// USDC whale address as per [FORK_BLOCK_MAINNET].
const USDC_WHALE_MAINNET: H160 = H160(hex_literal::hex!(
    "28c6c06298d514db089934071355e5743bf21d60"
));

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_single_limit_order() {
    run_forked_test_with_block_number(
        forked_mainnet_single_limit_order_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

const FORK_BLOCK_GNOSIS: u64 = 32070725;
/// USDC whale address as per [FORK_BLOCK_GNOSIS].
const USDC_WHALE_GNOSIS: H160 = H160(hex_literal::hex!(
    "ba12222222228d8ba445958a75a0704d566bf2c8"
));

#[tokio::test]
#[ignore]
async fn forked_node_gnosis_single_limit_order() {
    run_forked_test_with_block_number(
        forked_gnosis_single_limit_order_test,
        std::env::var("FORK_URL_GNOSIS").expect("FORK_URL_GNOSIS must be set to run forked tests"),
        FORK_BLOCK_GNOSIS,
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
        sell_amount: to_wei(10).into(),
        buy_token: token_b.address(),
        buy_amount: to_wei(5).into(),
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
        sell_amount: to_wei(10).into(),
        buy_token: token_b.address(),
        buy_amount: to_wei(5).into(),
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
        sell_amount: to_wei(5).into(),
        buy_token: token_a.address(),
        buy_amount: to_wei(2).into(),
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
        sell_amount: to_wei(10).into(),
        buy_token: token_b.address(),
        buy_amount: to_wei(5).into(),
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
        sell_amount: to_wei(5).into(),
        fee_amount: to_wei(1).into(),
        buy_token: token_a.address(),
        buy_amount: to_wei(2).into(),
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

    let [solver] = onchain.make_solvers(to_wei(1)).await;
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
    let solver_endpoint =
        colocation::start_baseline_solver(onchain.contracts().weth.address()).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![colocation::SolverEngine {
            name: "test_solver".into(),
            account: solver,
            endpoint: solver_endpoint,
        }],
    );
    services
        .start_api(vec![
            "--max-limit-orders-per-user=1".into(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(1).into(),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(1).into(),
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
        sell_amount: to_wei(1).into(),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(2).into(),
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

async fn forked_mainnet_single_limit_order_test(web3: Web3) {
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
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_MAINNET)
        .await
        .unwrap();
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
        sell_amount: to_wei_with_exp(1000, 6).into(),
        buy_token: token_usdt.address(),
        buy_amount: to_wei_with_exp(500, 6).into(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    // Warm up co-located driver by quoting the order (otherwise placing an order
    // may time out)
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: token_usdc.address(),
            buy_token: token_usdt.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: to_wei_with_exp(1000, 6).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;

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

async fn forked_gnosis_single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;
    let forked_node_api = web3.api::<ForkedNodeApi<_>>();

    let [solver] = onchain.make_solvers_forked(to_wei(1)).await;

    let [trader] = onchain.make_accounts(to_wei(1)).await;

    let token_usdc = ERC20::at(
        &web3,
        "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
            .parse()
            .unwrap(),
    );

    let token_wxdai = ERC20::at(
        &web3,
        "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d"
            .parse()
            .unwrap(),
    );

    // Give trader some USDC
    let usdc_whale = forked_node_api
        .impersonate(&USDC_WHALE_GNOSIS)
        .await
        .unwrap();
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
        sell_amount: to_wei_with_exp(1000, 6).into(),
        buy_token: token_wxdai.address(),
        buy_amount: to_wei(500).into(),
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
    let buy_token_balance_before = token_wxdai
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
    let buy_token_balance_after = token_wxdai
        .balance_of(trader.address())
        .call()
        .await
        .unwrap();

    assert!(sell_token_balance_before > sell_token_balance_after);
    assert!(buy_token_balance_after >= buy_token_balance_before + to_wei(500));
}
