use {
    crate::database::AuctionTransaction,
    ::alloy::{
        primitives::{Address, U256, address},
        providers::ext::{AnvilApi, ImpersonateConfig},
    },
    bigdecimal::BigDecimal,
    contracts::alloy::ERC20,
    database::byte_array::ByteArray,
    driver::domain::eth::NonZeroU256,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    fee::{FeePolicyOrderClass, ProtocolFee, ProtocolFeesConfig},
    model::{
        order::{OrderClass, OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{conversions::big_decimal_to_big_uint, units::EthUnit},
    shared::ethrpc::Web3,
    std::{collections::HashMap, ops::DerefMut},
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

/// Test that orders with haircut configured still execute on-chain.
/// The haircut reduces the reported surplus but the order should still be
/// fillable and execute successfully.
#[tokio::test]
#[ignore]
async fn local_node_limit_order_with_haircut() {
    run_test(limit_order_with_haircut_test).await;
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 23112197;
/// USDC whale address as per [FORK_BLOCK_MAINNET].
const USDC_WHALE_MAINNET: Address = address!("28c6c06298d514db089934071355e5743bf21d60");

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

const FORK_BLOCK_GNOSIS: u64 = 41502478;
/// USDC whale address as per [FORK_BLOCK_GNOSIS].
const USDC_WHALE_GNOSIS: Address = address!("d4A39d219ADB43aB00739DC5D876D98Fdf0121Bf");

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

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), 10u64.eth()).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), 1000u64.eth()).await;
    token_b.mint(solver.address(), 1000u64.eth()).await;
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            1000u64.eth(),
            1000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let balance_before = token_b.balanceOf(trader_a.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();

    // we hide the quote's execution plan while the order is still fillable
    let order = services.get_order(&order_id).await.unwrap();
    assert_eq!(
        order.metadata.quote.unwrap().metadata,
        serde_json::Value::default()
    );

    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_b.balanceOf(trader_a.address()).call().await.unwrap();
        balance_after.checked_sub(balance_before).unwrap() >= 5u64.eth()
    })
    .await
    .unwrap();

    wait_for_condition(TIMEOUT, || async {
        // after the order got filled we are able to see the quote's execution plan
        let order = services.get_order(&order_id).await.unwrap();
        tracing::error!(?order);
        order.metadata.quote.unwrap().metadata != serde_json::Value::default()
    })
    .await
    .unwrap();
}

async fn two_limit_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts and prepare funding Uniswap pool
    token_a.mint(trader_a.address(), 10u64.eth()).await;
    token_b.mint(trader_b.address(), 10u64.eth()).await;
    token_a.mint(solver.address(), 1_000u64.eth()).await;
    token_b.mint(solver.address(), 1_000u64.eth()).await;

    // Create and fund Uniswap pool
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            1000u64.eth(),
            1000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order_a = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );

    let balance_before_a = token_b.balanceOf(trader_a.address()).call().await.unwrap();
    let balance_before_b = token_a.balanceOf(trader_b.address()).call().await.unwrap();

    let order_id = services.create_order(&order_a).await.unwrap();
    onchain.mint_block().await;

    let limit_order = services.get_order(&order_id).await.unwrap();
    assert!(limit_order.metadata.class.is_limit());

    let order_b = OrderCreation {
        sell_token: *token_b.address(),
        sell_amount: 5u64.eth(),
        buy_token: *token_a.address(),
        buy_amount: 2u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::EthSign,
        &onchain.contracts().domain_separator,
        &trader_b.signer,
    );
    let order_id = services.create_order(&order_b).await.unwrap();

    let limit_order = services.get_order(&order_id).await.unwrap();
    assert!(limit_order.metadata.class.is_limit());

    // Drive solution
    tracing::info!("Waiting for trade.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after_a = token_b.balanceOf(trader_a.address()).call().await.unwrap();
        let balance_after_b = token_a.balanceOf(trader_b.address()).call().await.unwrap();
        let order_a_settled = balance_after_a.saturating_sub(balance_before_a) >= 5u64.eth();
        let order_b_settled = balance_after_b.saturating_sub(balance_before_b) >= 2u64.eth();
        order_a_settled && order_b_settled
    })
    .await
    .unwrap();
}

async fn two_limit_orders_multiple_winners_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver_a, solver_b] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b, token_c, token_d] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund traders
    token_a.mint(trader_a.address(), 10u64.eth()).await;
    token_b.mint(trader_b.address(), 10u64.eth()).await;

    // Create more liquid routes between token_a (token_b) and weth via base_a
    // (base_b). base_a has more liquidity than base_b, leading to the solver that
    // knows about base_a to offer different solution.
    let [base_a, base_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(100_000u64.eth(), 100_000u64.eth())
        .await;
    onchain
        .seed_uni_v2_pool((&token_a, 100_000u64.eth()), (&base_a, 100_000u64.eth()))
        .await;
    onchain
        .seed_uni_v2_pool((&token_b, 10_000u64.eth()), (&base_b, 10_000u64.eth()))
        .await;

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(onchain.contracts().allowance, 100u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start system, with two solvers, one that knows about base_a and one that
    // knows about base_b
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver_a.clone(),
                *onchain.contracts().weth.address(),
                vec![*base_a.address()],
                2,
                false,
            )
            .await,
            colocation::start_baseline_solver(
                "solver2".into(),
                solver_b.clone(),
                *onchain.contracts().weth.address(),
                vec![*base_b.address()],
                2,
                false,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    let services = Services::new(&onchain).await;
    services
        .start_api(vec![
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
            "--native-price-estimators=Driver|test_quoter|http://localhost:11088/test_solver"
                .to_string(),
        ])
        .await;

    // Place Orders
    let order_a = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_c.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let uid_a = services.create_order(&order_a).await.unwrap();

    let order_b = OrderCreation {
        sell_token: *token_b.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_d.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_b.signer,
    );
    let uid_b = services.create_order(&order_b).await.unwrap();

    // Start autopilot only once all the orders are created.
    services.start_autopilot(
        None,
        vec![
            format!("--drivers=solver1|http://localhost:11088/test_solver|{}|10000000000000000,solver2|http://localhost:11088/solver2|{}",
            const_hex::encode(solver_a.address()), const_hex::encode(solver_b.address())),
            "--price-estimation-drivers=solver1|http://localhost:11088/test_solver".to_string(),
            "--max-winners-per-auction=2".to_string(),
        ],
    ).await;

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
    assert!(order_a_settled.metadata.executed_fee > U256::ZERO);
    let order_b_settled = services.get_order(&uid_b).await.unwrap();
    assert!(order_b_settled.metadata.executed_fee > U256::ZERO);

    let mut ex = services.db().acquire().await.unwrap();
    let solver_a_winning_solutions =
        database::solver_competition_v2::fetch_solver_winning_solutions(
            &mut ex,
            competition.auction_id,
            ByteArray(solver_a.address().0.0),
        )
        .await
        .unwrap();
    let solver_b_winning_solutions =
        database::solver_competition_v2::fetch_solver_winning_solutions(
            &mut ex,
            competition.auction_id,
            ByteArray(solver_b.address().0.0),
        )
        .await
        .unwrap();
    assert_eq!(solver_a_winning_solutions.len(), 1);
    assert_eq!(solver_b_winning_solutions.len(), 1);
    assert_eq!(solver_a_winning_solutions[0].orders.len(), 1);
    assert_eq!(solver_b_winning_solutions[0].orders.len(), 1);
    let solver_a_order = solver_a_winning_solutions[0].orders[0].clone();
    assert_eq!(solver_a_order.uid.0, order_a_settled.metadata.uid.0);
    assert_eq!(
        big_decimal_to_big_uint(&solver_a_order.executed_sell).unwrap(),
        order_a_settled.metadata.executed_sell_amount
    );
    assert_eq!(
        big_decimal_to_big_uint(&solver_a_order.executed_buy).unwrap(),
        order_a_settled.metadata.executed_buy_amount
    );
    let solver_order_b = solver_b_winning_solutions[0].orders[0].clone();
    assert_eq!(solver_order_b.uid.0, order_b_settled.metadata.uid.0);
    assert_eq!(
        big_decimal_to_big_uint(&solver_order_b.executed_sell).unwrap(),
        order_b_settled.metadata.executed_sell_amount
    );
    assert_eq!(
        big_decimal_to_big_uint(&solver_order_b.executed_buy).unwrap(),
        order_b_settled.metadata.executed_buy_amount
    );

    let settlements_query = "SELECT * FROM settlements WHERE auction_id = $1";
    let settlements: Vec<AuctionTransaction> = sqlx::query_as(settlements_query)
        .bind(competition.auction_id)
        .fetch_all(ex.deref_mut())
        .await
        .unwrap();
    assert_eq!(settlements.len(), 2);
    assert!(settlements.iter().any(|settlement| settlement.solver
        == ByteArray(solver_a.address().0.0)
        && settlement.solution_uid == solver_a_winning_solutions[0].uid));
    assert!(settlements.iter().any(|settlement| settlement.solver
        == ByteArray(solver_b.address().0.0)
        && settlement.solution_uid == solver_b_winning_solutions[0].uid));

    // Ensure all the reference scores are indexed
    let reference_scores: HashMap<database::Address, BigDecimal> =
        database::reference_scores::fetch(&mut ex, competition.auction_id)
            .await
            .unwrap()
            .into_iter()
            .map(|score| (score.solver, score.reference_score))
            .collect();
    assert_eq!(reference_scores.len(), 2);

    // fetch the reference scores of both winners
    let solver_a_reference_score = reference_scores
        .get(&ByteArray(solver_a.address().0.0))
        .unwrap()
        .clone();
    let solver_b_reference_score = reference_scores
        .get(&ByteArray(solver_b.address().0.0))
        .unwrap()
        .clone();

    // The expected reference score for each winner is the solution score of the
    // other winner
    let solver_a_solution_score = solver_a_winning_solutions[0].score.clone();
    let solver_b_solution_score = solver_b_winning_solutions[0].score.clone();
    assert_eq!(solver_a_reference_score, solver_b_solution_score);
    assert_eq!(solver_b_reference_score, solver_a_solution_score);
}

async fn too_many_limit_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let solver_address = solver.address();
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    token_a.mint(trader.address(), 1u64.eth()).await;

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 101u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver,
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=test_solver|http://localhost:11088/test_solver|{}",
                    const_hex::encode(solver_address)
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--max-limit-orders-per-user=1".into(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    services.create_order(&order).await.unwrap();

    // Attempt to place another order, but the orderbook is configured to allow only
    // one limit order per user.
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 1u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 2u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let (status, body) = services.create_order(&order).await.unwrap_err();
    assert_eq!(status, 400);
    assert!(body.contains("TooManyLimitOrders"));
}

async fn limit_does_not_apply_to_in_market_orders_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let solver_address = solver.address();
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    token.mint(trader.address(), 100u64.eth()).await;

    // Approve GPv2 for trading

    token
        .approve(onchain.contracts().allowance, 101u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver,
                *onchain.contracts().weth.address(),
                vec![],
                1,
                true,
            )
            .await,
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );
    services
        .start_autopilot(
            None,
            vec![
                format!(
                    "--drivers=test_solver|http://localhost:11088/test_solver|{}",
                    const_hex::encode(solver_address)
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
            ],
        )
        .await;
    services
        .start_api(vec![
            "--max-limit-orders-per-user=1".into(),
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
        ])
        .await;

    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *token.address(),
        buy_token: *onchain.contracts().weth.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(5u64.eth()).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();

    // Place "in-market" order
    let order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: quote.quote.sell_amount,
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: quote.quote.buy_amount.saturating_sub(4u64.eth()),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    assert!(services.create_order(&order).await.is_ok());

    // Place a "limit" order
    let order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 1u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 3u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert!(limit_order.metadata.class.is_limit());

    // Place another "in-market" order in order to check it is not limited
    let order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: quote.quote.sell_amount,
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: quote.quote.buy_amount.saturating_sub(2u64.eth()),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    assert!(services.create_order(&order).await.is_ok());

    // Place a "limit" order in order to see if fails
    let order = OrderCreation {
        sell_token: *token.address(),
        sell_amount: 1u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 2u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let (status, body) = services.create_order(&order).await.unwrap_err();
    assert_eq!(status, 400);
    assert!(body.contains("TooManyLimitOrders"));
}

async fn forked_mainnet_single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let token_usdc = ERC20::Instance::new(
        address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        web3.alloy.clone(),
    );

    let token_usdt = ERC20::Instance::new(
        address!("dac17f958d2ee523a2206206994597c13d831ec7"),
        web3.alloy.clone(),
    );

    // Give trader some USDC
    web3.alloy
        .anvil_send_impersonated_transaction_with_config(
            token_usdc
                .transfer(trader.address(), 1000u64.matom())
                .from(USDC_WHALE_MAINNET)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Approve GPv2 for trading
    token_usdc
        .approve(onchain.contracts().allowance, 1000u64.matom())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    let order = OrderCreation {
        sell_token: *token_usdc.address(),
        sell_amount: 1000u64.matom(),
        buy_token: *token_usdt.address(),
        buy_amount: 500u64.matom(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    // Warm up co-located driver by quoting the order (otherwise placing an order
    // may time out)
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: *token_usdc.address(),
            buy_token: *token_usdt.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1000u64.matom()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await;

    let sell_token_balance_before = token_usdc.balanceOf(trader.address()).call().await.unwrap();
    let buy_token_balance_before = token_usdt.balanceOf(trader.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let sell_token_balance_after = token_usdc.balanceOf(trader.address()).call().await.unwrap();
        let buy_token_balance_after = token_usdt.balanceOf(trader.address()).call().await.unwrap();

        (sell_token_balance_before > sell_token_balance_after)
            && (buy_token_balance_after >= buy_token_balance_before + 500u64.matom())
    })
    .await
    .unwrap();
}

async fn forked_gnosis_single_limit_order_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    let [trader] = onchain.make_accounts(1u64.eth()).await;

    let token_usdc = ERC20::Instance::new(
        address!("ddafbb505ad214d7b80b1f830fccc89b60fb7a83"),
        web3.alloy.clone(),
    );

    let token_wxdai = ERC20::Instance::new(
        address!("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
        web3.alloy.clone(),
    );

    // Give trader some USDC
    web3.alloy
        .anvil_send_impersonated_transaction_with_config(
            token_usdc
                .transfer(trader.address(), 1000u64.matom())
                .from(USDC_WHALE_GNOSIS)
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: None,
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Approve GPv2 for trading
    token_usdc
        .approve(onchain.contracts().allowance, 1000u64.matom())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let order = OrderCreation {
        sell_token: *token_usdc.address(),
        sell_amount: 1000u64.matom(),
        buy_token: *token_wxdai.address(),
        buy_amount: 500u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let sell_token_balance_before = token_usdc.balanceOf(trader.address()).call().await.unwrap();
    let buy_token_balance_before = token_wxdai
        .balanceOf(trader.address())
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
        let sell_token_balance_after = token_usdc.balanceOf(trader.address()).call().await.unwrap();
        let buy_token_balance_after = token_wxdai
            .balanceOf(trader.address())
            .call()
            .await
            .unwrap();

        (sell_token_balance_before > sell_token_balance_after)
            && (buy_token_balance_after >= buy_token_balance_before + 500u64.eth())
    })
    .await
    .unwrap();
}

async fn no_liquidity_limit_order(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10_000u64.eth()).await;
    let [trader_a] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, unsupported] = onchain.deploy_tokens(solver.address()).await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), 10u64.eth()).await;

    // Approve GPv2 for trading

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    // Setup services
    let protocol_fee_args = ProtocolFeesConfig {
        protocol_fees: vec![
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
        ],
        ..Default::default()
    }
    .into_args();

    let services = Services::new(&onchain).await;
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                autopilot: [
                    protocol_fee_args,
                    vec![format!("--unsupported-tokens={:#x}", unsupported.address())],
                ]
                .concat(),
                ..Default::default()
            },
            solver,
        )
        .await;

    // Place order
    let mut order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *onchain.contracts().weth.address(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let order_id = services.create_order(&order).await.unwrap();
    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Cannot place orders with unsupported tokens
    order.sell_token = *unsupported.address();
    services
        .create_order(&order.sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader_a.signer,
        ))
        .await
        .unwrap_err();

    let balance_before = onchain
        .contracts()
        .weth
        .balanceOf(trader_a.address())
        .call()
        .await
        .unwrap();

    // Create liquidity
    onchain
        .seed_weth_uni_v2_pools([&token_a].iter().copied(), 1000u64.eth(), 1000u64.eth())
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
    assert_eq!(fee.token, *onchain.contracts().weth.address());
    assert!(fee.amount > ::alloy::primitives::U256::ZERO);

    let balance_after = onchain
        .contracts()
        .weth
        .balanceOf(trader_a.address())
        .call()
        .await
        .unwrap();
    assert!(balance_after.checked_sub(balance_before).unwrap() >= 5u64.eth());
}

/// Test that a limit order with haircut configured still executes on-chain.
/// The haircut adjusts clearing prices to report lower surplus, but the order
/// should still be fillable since the limit price allows for enough slack.
async fn limit_order_with_haircut_test(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader_a] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Fund trader accounts
    token_a.mint(trader_a.address(), 10u64.eth()).await;

    // Create and fund Uniswap pool
    token_a.mint(solver.address(), 1000u64.eth()).await;
    token_b.mint(solver.address(), 1000u64.eth()).await;
    onchain
        .contracts()
        .uniswap_v2_factory
        .createPair(*token_a.address(), *token_b.address())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_b
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .uniswap_v2_router
        .addLiquidity(
            *token_a.address(),
            *token_b.address(),
            1000u64.eth(),
            1000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading
    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();

    // Place Orders
    let services = Services::new(&onchain).await;
    // Start protocol with 500 bps (5%) haircut
    services
        .start_protocol_with_args_and_haircut(Default::default(), solver, 500)
        .await;

    // Create order with generous limit to ensure there's slack for haircut
    // Sell 10 A for at least 5 B (pool has 1:1 ratio so we'd get ~9.9 B without
    // fees)
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 5u64.eth(), // Generous limit creates slack for haircut
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let trader_balance_before = token_b.balanceOf(trader_a.address()).call().await.unwrap();
    let settlement_balance_before = token_b
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();
    let order_id = services.create_order(&order).await.unwrap();

    onchain.mint_block().await;
    let limit_order = services.get_order(&order_id).await.unwrap();
    assert_eq!(limit_order.metadata.class, OrderClass::Limit);

    // Drive solution - order should execute even with haircut applied
    tracing::info!("Waiting for trade with haircut.");
    wait_for_condition(TIMEOUT, || async {
        let balance_after = token_b.balanceOf(trader_a.address()).call().await.unwrap();
        balance_after.checked_sub(trader_balance_before).unwrap() >= 5u64.eth()
    })
    .await
    .unwrap();

    // Verify that haircut (positive slippage) remains in the settlement contract.
    // The haircut is 500 bps (5%) of the executed sell amount (10 ETH).
    // At 1:1 pool ratio, this is approximately 0.5 ETH worth of token_b.
    let trader_balance_after = token_b.balanceOf(trader_a.address()).call().await.unwrap();
    let settlement_balance_after = token_b
        .balanceOf(*onchain.contracts().gp_settlement.address())
        .call()
        .await
        .unwrap();

    let trader_received = trader_balance_after
        .checked_sub(trader_balance_before)
        .unwrap();
    let settlement_received = settlement_balance_after
        .checked_sub(settlement_balance_before)
        .unwrap();

    // Expected haircut: 5% of 10 ETH sell amount = 0.5 ETH (in buy token terms at
    // ~1:1 ratio). Allow some tolerance for fees and rounding.
    assert!(
        settlement_received >= 0.4.eth() && settlement_received <= 0.6.eth(),
        "Settlement contract should have received haircut (positive slippage) between 0.4 and 0.6 \
         ETH, but got {}",
        settlement_received
    );

    // Expected trader amount: output (~9.87 ETH at 1:1 ratio with 0.3% fee)
    // minus haircut (~0.5 ETH) = ~9.37 ETH. Allow tolerance for rounding.
    assert!(
        trader_received >= 9u64.eth() && trader_received <= 9.5.eth(),
        "Trader should have received between 9 and 9.5 ETH (AMM output minus haircut), but got {}",
        trader_received
    );

    // Wait for solver competition data to be indexed
    tracing::info!("Waiting for solver competition to be indexed");
    let indexed = || async {
        onchain.mint_block().await;
        match services.get_trades(&order_id).await.unwrap().first() {
            Some(trade) => services
                .get_solver_competition(trade.tx_hash.unwrap())
                .await
                .is_ok(),
            None => false,
        }
    };
    wait_for_condition(TIMEOUT, indexed).await.unwrap();

    let trades = services.get_trades(&order_id).await.unwrap();
    let tx_hash = trades[0].tx_hash.unwrap();
    let competition = services.get_solver_competition(tx_hash).await.unwrap();

    // Find our order in the winning solution
    let winner = competition
        .solutions
        .iter()
        .find(|s| s.is_winner)
        .expect("Should have winning solution");

    let reported_order = winner
        .orders
        .iter()
        .find(|o| o.id == order_id)
        .expect("Order should be in solution");

    let signed_sell_amount = U256::from(order.sell_amount);
    let reported_sell_amount = reported_order.sell_amount;

    assert!(
        reported_sell_amount <= signed_sell_amount,
        "Driver reported sell_amount {} exceeds signed sell_amount {}. Haircut should reduce \
         surplus/score, not inflate the reported sell amount!",
        reported_sell_amount,
        signed_sell_amount
    );
}
