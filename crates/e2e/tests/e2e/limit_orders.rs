use {
    contracts::ERC20,
    driver::domain::eth::NonZeroU256,
    e2e::{nodes::forked_node::ForkedNodeApi, setup::*, tx},
    ethcontract::{prelude::U256, H160},
    fee::{FeePolicyOrderClass, ProtocolFee, ProtocolFeesConfig},
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
async fn local_node_two_limit_orders_multiple_winners() {
    run_test(two_limit_orders_multiple_winners_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_too_many_limit_orders() {
    run_test(too_many_limit_orders_test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_limit_does_not_apply_to_in_market_orders_test() {
    run_test(limit_does_not_apply_to_in_market_orders_test).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
#[ignore]
async fn local_node_no_liquidity_limit_order() {
    run_test(no_liquidity_limit_order).await;
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
    let services = Services::new(&onchain).await;
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
    let balance_before = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_b.balance_of(trader_a.address()).call().await.unwrap();
        balance_after.checked_sub(balance_before).unwrap() >= to_wei(5)
    })
    .await
    .unwrap();
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
    let services = Services::new(&onchain).await;
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

    let balance_before_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
    let balance_before_b = token_a.balance_of(trader_b.address()).call().await.unwrap();

    let order_id = services.create_order(&order_a).await.unwrap();
    onchain.mint_block().await;

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

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after_a = token_b.balance_of(trader_a.address()).call().await.unwrap();
        let balance_after_b = token_a.balance_of(trader_b.address()).call().await.unwrap();
        let order_a_settled = balance_after_a.saturating_sub(balance_before_a) >= to_wei(5);
        let order_b_settled = balance_after_b.saturating_sub(balance_before_b) >= to_wei(2);
        order_a_settled && order_b_settled
    })
    .await
    .unwrap();
}

async fn two_limit_orders_multiple_winners_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver_a, solver_b] = onchain.make_solvers(to_wei(1)).await;
    let [trader_a, trader_b] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, token_b, token_c, token_d] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    // Fund traders
    token_a.mint(trader_a.address(), to_wei(10)).await;
    token_b.mint(trader_b.address(), to_wei(10)).await;

    // Create more liquid routes between token_a (token_b) and weth via base_a
    // (base_b). base_a has more liquidity than base_b, leading to the solver that
    // knows about base_a to offer different solution.
    let [base_a, base_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(10_000), to_wei(10_000))
        .await;
    onchain
        .seed_uni_v2_pool((&token_a, to_wei(100_000)), (&base_a, to_wei(100_000)))
        .await;
    onchain
        .seed_uni_v2_pool((&token_b, to_wei(10_000)), (&base_b, to_wei(10_000)))
        .await;

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(100))
    );
    tx!(
        trader_b.account(),
        token_b.approve(onchain.contracts().allowance, to_wei(100))
    );

    // Start system, with two solvers, one that knows about base_a and one that
    // knows about base_b
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver_a.clone(),
                onchain.contracts().weth.address(),
                vec![base_a.address()],
                2,
                false,
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver_b,
                onchain.contracts().weth.address(),
                vec![base_b.address()],
                2,
                false,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );

    let services = Services::new(&onchain).await;
    services.start_autopilot(
        None,
        vec![
            "--drivers=solver1|http://localhost:11088/test_solver|10000000000000000,solver2|http://localhost:11088/solver2"
                .to_string(),
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
            "--max-winners-per-auction=2".to_string(),
        ],
    ).await;
    services
        .start_api(vec![
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    // Place Orders
    let order_a = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: token_c.address(),
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
    let uid_a = services.create_order(&order_a).await.unwrap();

    let order_b = OrderCreation {
        sell_token: token_b.address(),
        sell_amount: to_wei(10),
        buy_token: token_d.address(),
        buy_amount: to_wei(5),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader_b.private_key()).unwrap()),
    );
    let uid_b = services.create_order(&order_b).await.unwrap();

    // Wait for trade
    let indexed_trades = || async {
        onchain.mint_block().await;
        let trade_a = services.get_trades(&uid_a).await.unwrap().first().cloned();
        let trade_b = services.get_trades(&uid_b).await.unwrap().first().cloned();
        match (trade_a, trade_b) {
            (Some(trade_a), Some(trade_b)) => {
                matches!(
                    (
                        services
                            .get_solver_competition(trade_a.tx_hash.unwrap())
                            .await,
                        services
                            .get_solver_competition(trade_b.tx_hash.unwrap())
                            .await
                    ),
                    (Ok(_), Ok(_))
                )
            }
            _ => false,
        }
    };
    wait_for_condition(TIMEOUT, indexed_trades).await.unwrap();

    let trades = services.get_trades(&uid_a).await.unwrap();
    let competition = services
        .get_solver_competition(trades[0].tx_hash.unwrap())
        .await
        .unwrap();
    // Verify that both transactions were properly indexed
    assert_eq!(competition.transaction_hashes.len(), 2);
    // Verify that settlement::Observed properly handled events
    let order_a_settled = services.get_order(&uid_a).await.unwrap();
    assert!(order_a_settled.metadata.executed_surplus_fee > 0.into());
    let order_b_settled = services.get_order(&uid_b).await.unwrap();
    assert!(order_b_settled.metadata.executed_surplus_fee > 0.into());
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
    let services = Services::new(&onchain).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver,
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );
    services
        .start_api(vec![
            "--max-limit-orders-per-user=1".into(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
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

async fn limit_does_not_apply_to_in_market_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(1)).await;
    let [trader] = onchain.make_accounts(to_wei(1)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;
    token.mint(trader.address(), to_wei(100)).await;

    // Approve GPv2 for trading
    tx!(
        trader.account(),
        token.approve(onchain.contracts().allowance, to_wei(101))
    );

    // Place Orders
    let services = Services::new(&onchain).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver,
                onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
    );
    services
        .start_api(vec![
            "--max-limit-orders-per-user=1".into(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: token.address(),
        buy_token: onchain.contracts().weth.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(to_wei(5)).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();

    // Place "in-market" order
    let order = OrderCreation {
        sell_token: token.address(),
        sell_amount: quote.quote.sell_amount,
        buy_token: onchain.contracts().weth.address(),
        buy_amount: quote.quote.buy_amount.saturating_sub(to_wei(4)),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    assert!(services.create_order(&order).await.is_ok());

    // Place a "limit" order
    let order = OrderCreation {
        sell_token: token.address(),
        sell_amount: to_wei(1),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(3),
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
    assert!(limit_order.metadata.class.is_limit());

    // Place another "in-market" order in order to check it is not limited
    let order = OrderCreation {
        sell_token: token.address(),
        sell_amount: quote.quote.sell_amount,
        buy_token: onchain.contracts().weth.address(),
        buy_amount: quote.quote.buy_amount.saturating_sub(to_wei(2)),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
    assert!(services.create_order(&order).await.is_ok());

    // Place a "limit" order in order to see if fails
    let order = OrderCreation {
        sell_token: token.address(),
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
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

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
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
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

        (sell_token_balance_before > sell_token_balance_after)
            && (buy_token_balance_after >= buy_token_balance_before + to_wei_with_exp(500, 6))
    })
    .await
    .unwrap();
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
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: token_usdc.address(),
        sell_amount: to_wei_with_exp(1000, 6),
        buy_token: token_wxdai.address(),
        buy_amount: to_wei(500),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );
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
    let order_id = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
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

        (sell_token_balance_before > sell_token_balance_after)
            && (buy_token_balance_after >= buy_token_balance_before + to_wei(500))
    })
    .await
    .unwrap();
}

async fn no_liquidity_limit_order(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(to_wei(10_000)).await;
    let [trader_a] = onchain.make_accounts(to_wei(1)).await;
    let [token_a, unsupported] = onchain.deploy_tokens(solver.account()).await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), to_wei(10)).await;

    // Approve GPv2 for trading
    tx!(
        trader_a.account(),
        token_a.approve(onchain.contracts().allowance, to_wei(10))
    );

    // Setup services
    let protocol_fees_config = ProtocolFeesConfig(vec![
        ProtocolFee {
            policy: fee::FeePolicyKind::Surplus {
                factor: 0.5,
                max_volume_factor: 0.01,
            },
            policy_order_class: FeePolicyOrderClass::Limit,
        },
        ProtocolFee {
            policy: fee::FeePolicyKind::PriceImprovement {
                factor: 0.5,
                max_volume_factor: 0.01,
            },
            policy_order_class: FeePolicyOrderClass::Market,
        },
    ])
    .to_string();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: vec![
                    protocol_fees_config,
                    "--enable-multiple-fees=true".to_string(),
                    format!("--unsupported-tokens={:#x}", unsupported.address()),
                ],
                ..Default::default()
            },
            solver,
        )
        .await;

    // Place order
    let mut order = OrderCreation {
        sell_token: token_a.address(),
        sell_amount: to_wei(10),
        buy_token: onchain.contracts().weth.address(),
        buy_amount: to_wei(1),
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
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Cannot place orders with unsupported tokens
    order.sell_token = unsupported.address();
    services
        .create_order(&order.sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            SecretKeyRef::from(&SecretKey::from_slice(trader_a.private_key()).unwrap()),
        ))
        .await
        .unwrap_err();

    let balance_before = onchain
        .contracts()
        .weth
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap();

    // Create liquidity
    onchain
        .seed_weth_uni_v2_pools([&token_a].iter().copied(), to_wei(1000), to_wei(1000))
        .await;

    // Drive solution
    tracing::info!("Waiting for trade.");

    // wait for trade to be indexed and post-processed
    wait_for_condition(TIMEOUT, || async {
        // Keep minting blocks to eventually invalidate the liquidity cached by the
        // driver making it refetch the current state which allows it to finally compute
        // a solution.
        onchain.mint_block().await;
        services
            .get_trades(&order_id)
            .await
            .unwrap()
            .first()
            .is_some_and(|t| !t.executed_protocol_fees.is_empty())
    })
    .await
    .unwrap();

    let trade = services.get_trades(&order_id).await.unwrap().pop().unwrap();
    let fee = trade.executed_protocol_fees.first().unwrap();
    assert_eq!(
        fee.policy,
        model::fee_policy::FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.01
        }
    );
    assert_eq!(fee.token, onchain.contracts().weth.address());
    assert!(fee.amount > 0.into());

    let balance_after = onchain
        .contracts()
        .weth
        .balance_of(trader_a.address())
        .call()
        .await
        .unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= to_wei(5));
}
