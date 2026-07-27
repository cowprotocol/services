use {
    ::alloy::primitives::U256,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
    std::time::Duration,
};

#[tokio::test]
#[ignore]
async fn local_node_valid_from_gates_auction_entry() {
    run_test(valid_from_gates_auction_entry).await;
}

/// An order whose app-data sets a future `validFrom` must not enter the
/// autopilot's auction until `now >= validFrom`, after which it is picked up
/// and settled like any other order.
async fn valid_from_gates_auction_entry(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(1u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token_a, token_b] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token_a.mint(trader.address(), 10u64.eth()).await;
    token_a.mint(solver.address(), 1_000u64.eth()).await;
    token_b.mint(solver.address(), 1_000u64.eth()).await;

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
            1_000u64.eth(),
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();
    token_b
        .approve(
            *onchain.contracts().uniswap_v2_router.address(),
            1_000u64.eth(),
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
            1_000u64.eth(),
            1_000u64.eth(),
            U256::ZERO,
            U256::ZERO,
            solver.address(),
            U256::MAX,
        )
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    token_a
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Gate the order behind a `validFrom` a fixed window in the future.
    const GATE_SECONDS: u32 = 15;
    let valid_from = model::time::now_in_epoch_seconds() + GATE_SECONDS;
    let order = OrderCreation {
        sell_token: *token_a.address(),
        sell_amount: 10u64.eth(),
        buy_token: *token_b.address(),
        buy_amount: 5u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: format!(r#"{{"metadata":{{"validFrom":{valid_from}}}}}"#),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let balance_before = token_b.balanceOf(trader.address()).call().await.unwrap();
    let order_id = services.create_order(&order).await.unwrap();

    // The order is accepted and open, just not yet eligible for the auction.
    onchain.mint_block().await;
    assert_eq!(
        services.get_order(&order_id).await.unwrap().metadata.status,
        OrderStatus::Open,
    );

    // For the first part of the gating window the order must neither enter the
    // auction nor settle. An equivalent ungated order settles within a couple of
    // seconds, so sustained absence here is the gating, not latency. The balance
    // guard also catches a broken gate that settled and already left the auction.
    let gate_until = std::time::Instant::now() + Duration::from_secs((GATE_SECONDS / 2) as u64);
    while std::time::Instant::now() < gate_until {
        onchain.mint_block().await;
        assert!(
            services.get_auction().await.auction.orders.is_empty(),
            "gated order entered the auction before validFrom",
        );
        let balance = token_b.balanceOf(trader.address()).call().await.unwrap();
        assert_eq!(
            balance, balance_before,
            "gated order settled before validFrom"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Once validFrom passes, the order is picked up and settles.
    tracing::info!("waiting for the gated order to settle after validFrom");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let balance = token_b.balanceOf(trader.address()).call().await.unwrap();
        balance.checked_sub(balance_before).unwrap() >= 5u64.eth()
    })
    .await
    .unwrap();
}
