use {
    bigdecimal::Zero,
    e2e::setup::{pod::PodTestClient, wait_for_condition, *},
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    pod_sdk::alloy_primitives::U256,
    shared::web3::Web3,
};

/// Basic pod test - single order, single solver.
/// Verifies the fundamental pod flow: bid submission, auction end, and winner
/// selection.
#[tokio::test]
#[ignore]
async fn pod_test_basic() {
    run_pod_test(pod_basic_test).await;
}

async fn pod_basic_test(web3: Web3) {
    tracing::info!("Setting up chain state for basic pod test.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!(?solver, "Created solver account");
    tracing::info!(?trader, "Created trader account");
    tracing::info!(token_address = ?token.address(), "Deployed test token with UniV2 pool");

    // Approve and deposit WETH for trader
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 3u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(3u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    tracing::info!("Trader approved and deposited 3 ETH as WETH");

    tracing::info!("Starting services with pod-enabled driver.");
    let services = Services::new(&onchain).await;
    services.start_protocol_with_pod(solver.clone()).await;
    tracing::info!("Services started - driver has pod config enabled");

    tracing::info!("Submitting quote request");
    let quote_sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(quote_sell_amount).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    tracing::info!(
        quote_id = ?quote_response.id,
        buy_amount = ?quote_response.quote.buy_amount,
        "Got quote response"
    );

    tracing::info!("Placing order");
    let order = OrderCreation {
        quote_id: quote_response.id,
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: quote_sell_amount,
        buy_token: *token.address(),
        buy_amount: quote_response.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let order_uid = services.create_order(&order).await.unwrap();
    tracing::info!(?order_uid, "Order created successfully");

    // Wait for order to appear in auction
    tracing::info!("Waiting for order to appear in auction...");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let auction = services.get_auction().await;
        let has_order = !auction.auction.orders.is_empty();
        if has_order {
            tracing::info!(
                auction_id = auction.id,
                num_orders = auction.auction.orders.len(),
                "Order appeared in auction"
            );
        }
        has_order
    })
    .await
    .expect("Order should appear in auction");

    // Now wait for trade to happen - this triggers the full auction flow including
    // pod
    tracing::info!("Waiting for trade execution (pod flow should trigger)...");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let order = services.get_order(&order_uid).await.unwrap();
        let executed = !order.metadata.executed_buy_amount.is_zero();
        if executed {
            tracing::info!(
                executed_buy_amount = ?order.metadata.executed_buy_amount,
                executed_sell_amount = ?order.metadata.executed_sell_amount,
                "Trade executed!"
            );
        }
        executed
    })
    .await
    .expect("Trade should execute");

    // Verify solver competition data from autopilot
    let competition = services
        .get_latest_solver_competition()
        .await
        .expect("Should have solver competition data");

    // Verify auction had our order
    assert_eq!(
        competition.auction.orders.len(),
        1,
        "Auction should have exactly 1 order"
    );

    // Verify a winner was selected by autopilot
    let winners: Vec<_> = competition
        .solutions
        .iter()
        .filter(|s| s.is_winner)
        .collect();
    assert_eq!(winners.len(), 1, "Should have exactly 1 winner");

    let autopilot_winner = winners[0];
    assert_eq!(
        autopilot_winner.solver_address,
        solver.address(),
        "Autopilot winner should be our solver"
    );
    assert!(
        !autopilot_winner.score.is_zero(),
        "Winner should have non-zero score"
    );

    // === POD NETWORK VERIFICATION ===
    tracing::info!(
        auction_id = competition.auction_id,
        "Querying pod network for bids..."
    );

    let pod_client = PodTestClient::new()
        .await
        .expect("Should be able to connect to pod network");

    let pod_bids = pod_client
        .fetch_bids(competition.auction_id)
        .await
        .expect("Should be able to fetch bids from pod network");

    tracing::info!(
        auction_id = competition.auction_id,
        num_pod_bids = pod_bids.len(),
        solver = %solver.address(),
        "Fetched bids from pod network"
    );

    // Verify driver submitted a bid to pod network
    assert!(
        !pod_bids.is_empty(),
        "Driver should have submitted at least 1 bid to pod network"
    );

    // Find the bid from our solver
    let solver_bid = pod_bids
        .iter()
        .find(|b| b.submission_address == solver.address());

    assert!(
        solver_bid.is_some(),
        "Our solver {} should have a bid in pod network",
        solver.address()
    );
    let solver_bid = solver_bid.unwrap();

    // Verify the autopilot winner matches the pod bid submitter
    assert_eq!(
        autopilot_winner.solver_address, solver_bid.submission_address,
        "Autopilot winner should match pod bid submitter"
    );

    tracing::info!(
        autopilot_winner = %autopilot_winner.solver_address,
        pod_bid_score = ?solver_bid.score,
        autopilot_score = ?autopilot_winner.score,
        "✓ Pod basic test verified: bid submitted, autopilot selected correct winner"
    );
}

/// Multi-order pod test - tests that multiple orders in a single auction
/// are properly handled by the pod flow and winner selection logic.
#[tokio::test]
#[ignore]
async fn pod_test_multi_order() {
    run_pod_test(pod_multi_order_test).await;
}

async fn pod_multi_order_test(web3: Web3) {
    tracing::info!("Setting up chain state for pod multi-order test.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader_a, trader_b] = onchain.make_accounts(10u64.eth()).await;
    // Deploy two tokens with separate pools for different trading pairs
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!(?solver, "Created solver account");
    tracing::info!(?trader_a, "Created trader A account");
    tracing::info!(?trader_b, "Created trader B account");
    tracing::info!(token_a = ?token_a.address(), token_b = ?token_b.address(), "Deployed test tokens");

    // Setup trader A: approve and deposit WETH
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 5u64.eth())
        .from(trader_a.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader_a.address())
        .value(5u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    tracing::info!("Trader A approved and deposited 5 ETH as WETH");

    // Setup trader B: approve and deposit WETH
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 5u64.eth())
        .from(trader_b.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader_b.address())
        .value(5u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    tracing::info!("Trader B approved and deposited 5 ETH as WETH");

    tracing::info!("Starting services with pod-enabled driver.");
    let services = Services::new(&onchain).await;
    services.start_protocol_with_pod(solver.clone()).await;
    tracing::info!("Services started - driver has pod config enabled");

    // Get quotes and create orders for both traders
    let sell_amount_a = 1u64.eth();
    let quote_a = OrderQuoteRequest {
        from: trader_a.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token_a.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount_a).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote_response_a = services.submit_quote(&quote_a).await.unwrap();
    tracing::info!(
        quote_id = ?quote_response_a.id,
        buy_amount = ?quote_response_a.quote.buy_amount,
        "Got quote for trader A (WETH -> token_a)"
    );

    let sell_amount_b = 2u64.eth();
    let quote_b = OrderQuoteRequest {
        from: trader_b.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token_b.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount_b).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote_response_b = services.submit_quote(&quote_b).await.unwrap();
    tracing::info!(
        quote_id = ?quote_response_b.id,
        buy_amount = ?quote_response_b.quote.buy_amount,
        "Got quote for trader B (WETH -> token_b)"
    );

    // Place order A
    tracing::info!("Placing order A");
    let order_a = OrderCreation {
        quote_id: quote_response_a.id,
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: sell_amount_a,
        buy_token: *token_a.address(),
        buy_amount: quote_response_a.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_a.signer,
    );
    let order_uid_a = services.create_order(&order_a).await.unwrap();
    tracing::info!(?order_uid_a, "Order A created");

    // Place order B
    tracing::info!("Placing order B");
    let order_b = OrderCreation {
        quote_id: quote_response_b.id,
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: sell_amount_b,
        buy_token: *token_b.address(),
        buy_amount: quote_response_b.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader_b.signer,
    );
    let order_uid_b = services.create_order(&order_b).await.unwrap();
    tracing::info!(?order_uid_b, "Order B created");

    // Wait for both orders to appear in auction
    tracing::info!("Waiting for both orders to appear in auction...");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let auction = services.get_auction().await;
        let num_orders = auction.auction.orders.len();
        if num_orders == 2 {
            tracing::info!(
                auction_id = auction.id,
                num_orders,
                "Both orders appeared in auction"
            );
        }
        num_orders == 2
    })
    .await
    .expect("Both orders should appear in auction");

    // Wait for both trades to execute - this triggers pod flow with multiple orders
    tracing::info!("Waiting for both trades to execute (pod multi-order flow)...");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let order_a_status = services.get_order(&order_uid_a).await.unwrap();
        let order_b_status = services.get_order(&order_uid_b).await.unwrap();

        let a_executed = !order_a_status.metadata.executed_buy_amount.is_zero();
        let b_executed = !order_b_status.metadata.executed_buy_amount.is_zero();

        if a_executed {
            tracing::info!(
                order = "A",
                executed_buy = ?order_a_status.metadata.executed_buy_amount,
                executed_sell = ?order_a_status.metadata.executed_sell_amount,
                "Order A executed"
            );
        }
        if b_executed {
            tracing::info!(
                order = "B",
                executed_buy = ?order_b_status.metadata.executed_buy_amount,
                executed_sell = ?order_b_status.metadata.executed_sell_amount,
                "Order B executed"
            );
        }

        a_executed && b_executed
    })
    .await
    .expect("Both trades should execute");

    // Verify solver competition data from autopilot
    let competition = services
        .get_latest_solver_competition()
        .await
        .expect("Should have solver competition data");

    // Verify auction had both orders
    assert_eq!(
        competition.auction.orders.len(),
        2,
        "Auction should have exactly 2 orders"
    );

    // Verify a winner was selected by autopilot
    let winners: Vec<_> = competition
        .solutions
        .iter()
        .filter(|s| s.is_winner)
        .collect();
    assert_eq!(winners.len(), 1, "Should have exactly 1 winner");

    let autopilot_winner = winners[0];
    assert_eq!(
        autopilot_winner.solver_address,
        solver.address(),
        "Autopilot winner should be our solver"
    );

    // Verify the winning solution contains both orders
    assert_eq!(
        autopilot_winner.orders.len(),
        2,
        "Winning solution should contain both orders"
    );

    // === POD NETWORK VERIFICATION ===
    tracing::info!(
        auction_id = competition.auction_id,
        "Querying pod network for multi-order auction..."
    );

    let pod_client = PodTestClient::new()
        .await
        .expect("Should be able to connect to pod network");

    let pod_bids = pod_client
        .fetch_bids(competition.auction_id)
        .await
        .expect("Should be able to fetch bids from pod network");

    assert!(
        !pod_bids.is_empty(),
        "Driver should have submitted bid to pod network"
    );

    let solver_bid = pod_bids
        .iter()
        .find(|b| b.submission_address == solver.address());
    assert!(
        solver_bid.is_some(),
        "Our solver should have bid in pod network"
    );

    tracing::info!(
        autopilot_winner = %autopilot_winner.solver_address,
        autopilot_score = ?autopilot_winner.score,
        pod_bids_count = pod_bids.len(),
        num_orders = autopilot_winner.orders.len(),
        "✓ Pod multi-order verified: bid submitted, both orders in winning solution"
    );
}

/// Multi-solver pod test - tests that multiple solvers competing in the same
/// auction have their bids properly submitted to pod and winner selection
/// works. Each solver knows about different liquidity routes, leading to
/// different scores.
#[tokio::test]
#[ignore]
async fn pod_test_multi_solver() {
    run_pod_test(pod_multi_solver_test).await;
}

async fn pod_multi_solver_test(web3: Web3) {
    tracing::info!("Setting up chain state for pod multi-solver test.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver_a, solver_b] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;

    // Deploy token with WETH pool
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!(?solver_a, "Created solver A account (no haircut)");
    tracing::info!(
        ?solver_b,
        "Created solver B account (with haircut -> lower score)"
    );
    tracing::info!(?trader, "Created trader account");
    tracing::info!(
        token = ?token.address(),
        "Deployed token with WETH pool - solvers differentiated by haircut"
    );

    // Setup trader: approve and deposit WETH
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 5u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(5u64.eth())
        .send_and_watch()
        .await
        .unwrap();
    tracing::info!("Trader approved and deposited 5 ETH as WETH");

    tracing::info!("Starting services with pod-enabled multi-solver driver.");
    let services = Services::new(&onchain).await;

    // Solver A: no haircut, Solver B: 1% haircut (100 bps) for score
    // differentiation
    services
        .start_protocol_with_pod_multi_solver(vec![(solver_a.clone(), 0), (solver_b.clone(), 100)])
        .await;
    tracing::info!("Services started - solver_a vs solver_b");

    // Get quote and create order
    let sell_amount = 1u64.eth();
    let quote = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote).await.unwrap();
    tracing::info!(
        quote_id = ?quote_response.id,
        buy_amount = ?quote_response.quote.buy_amount,
        "Got quote"
    );

    // Place order with 5% slippage tolerance to accommodate haircut differences
    let min_buy = quote_response.quote.buy_amount * U256::from(95) / U256::from(100);
    tracing::info!("Placing order");
    let order = OrderCreation {
        quote_id: quote_response.id,
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: min_buy,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let order_uid = services.create_order(&order).await.unwrap();
    tracing::info!(?order_uid, "Order created");

    // Wait for order to appear in auction
    tracing::info!("Waiting for order to appear in auction...");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let auction = services.get_auction().await;
        let has_order = !auction.auction.orders.is_empty();
        if has_order {
            tracing::info!(
                auction_id = auction.id,
                num_orders = auction.auction.orders.len(),
                "Order appeared in auction"
            );
        }
        has_order
    })
    .await
    .expect("Order should appear in auction");

    // Wait for trade to execute - this triggers pod flow with multiple solvers
    // competing
    tracing::info!("Waiting for trade execution (pod multi-solver competition)...");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let order_status = services.get_order(&order_uid).await.unwrap();
        let executed = !order_status.metadata.executed_buy_amount.is_zero();
        if executed {
            tracing::info!(
                executed_buy = ?order_status.metadata.executed_buy_amount,
                executed_sell = ?order_status.metadata.executed_sell_amount,
                "Trade executed"
            );
        }
        executed
    })
    .await
    .expect("Trade should execute");

    // Verify solver competition data from autopilot
    let competition = services
        .get_latest_solver_competition()
        .await
        .expect("Should have solver competition data");

    // Verify auction had our order
    assert_eq!(
        competition.auction.orders.len(),
        1,
        "Auction should have exactly 1 order"
    );

    // Verify we have multiple solutions from different solvers
    assert!(
        competition.solutions.len() >= 2,
        "Should have at least 2 solutions from different solvers"
    );

    // Verify exactly one winner was selected by autopilot
    let winners: Vec<_> = competition
        .solutions
        .iter()
        .filter(|s| s.is_winner)
        .collect();
    assert_eq!(winners.len(), 1, "Should have exactly 1 winner");

    let autopilot_winner = winners[0];
    assert!(
        !autopilot_winner.score.is_zero(),
        "Winner should have non-zero score"
    );

    // The winner should be one of our solvers
    let valid_solvers = [solver_a.address(), solver_b.address()];
    assert!(
        valid_solvers.contains(&autopilot_winner.solver_address),
        "Winner should be one of our solvers"
    );

    // === POD NETWORK VERIFICATION ===
    tracing::info!(
        auction_id = competition.auction_id,
        "Querying pod network for multi-solver auction..."
    );

    let pod_client = PodTestClient::new()
        .await
        .expect("Should be able to connect to pod network");

    let pod_bids = pod_client
        .fetch_bids(competition.auction_id)
        .await
        .expect("Should be able to fetch bids from pod network");

    // Verify both solvers submitted bids to pod network
    assert!(
        pod_bids.len() >= 2,
        "Both solvers should have submitted bids to pod network, got {}",
        pod_bids.len()
    );

    let solver_a_bid = pod_bids
        .iter()
        .find(|b| b.submission_address == solver_a.address());
    let solver_b_bid = pod_bids
        .iter()
        .find(|b| b.submission_address == solver_b.address());

    assert!(
        solver_a_bid.is_some(),
        "Solver A should have submitted bid to pod network"
    );
    assert!(
        solver_b_bid.is_some(),
        "Solver B should have submitted bid to pod network"
    );

    // Verify winner selection consistency
    let pod_winner = pod_bids.iter().max_by_key(|b| b.score).unwrap();
    assert_eq!(
        autopilot_winner.solver_address, pod_winner.submission_address,
        "Autopilot winner should match pod network winner"
    );

    tracing::info!(
        solver_a = %solver_a.address(),
        solver_a_score = ?solver_a_bid.unwrap().score,
        solver_b = %solver_b.address(),
        solver_b_score = ?solver_b_bid.unwrap().score,
        pod_winner = %pod_winner.submission_address,
        autopilot_winner = %autopilot_winner.solver_address,
        "✓ Pod multi-solver verified: both solvers submitted bids, winner selection consistent"
    );
}
