use {
    bigdecimal::Zero,
    e2e::setup::{wait_for_condition, *},
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn pod_test_shadow_mode() {
    run_pod_test(pod_shadow_mode_test).await;
}

async fn pod_shadow_mode_test(web3: Web3) {
    tracing::info!("Setting up chain state for pod shadow mode test.");
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

    // Now wait for trade to happen - this triggers the full auction flow including pod
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

    tracing::info!("Pod shadow mode test completed successfully!");
    tracing::info!("Check logs above for [pod] entries showing:");
    tracing::info!("  - [pod] pod provider built with wallet");
    tracing::info!("  - [pod] preparing bid submission payload");
    tracing::info!("  - [pod] bid submission succeeded/failed");
    tracing::info!("  - [pod] auction ended");
    tracing::info!("  - [pod] fetched bids");
    tracing::info!("  - [pod] local arbitration completed");
}
