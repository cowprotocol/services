use {
    crate::ethflow::ExtendedEthFlowOrder,
    app_data::AppDataHash,
    configs::test_util::TestDefault,
    database::byte_array::ByteArray,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{
            OrderQuoteRequest,
            OrderQuoteSide,
            PriceQuality,
            QuoteSigningScheme,
            SellAmount,
            Validity,
        },
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    shared::web3::Web3,
    std::{ops::DerefMut, time::Duration},
};

#[tokio::test]
#[ignore]
async fn local_node_fast_path_quote_promotion() {
    run_test(fast_path_quote_promotion).await;
}

/// End-to-end check that a fast-path quote's competition rows are written at
/// quote time and then re-keyed to the real `order_uid` when the order is
/// placed.
async fn fast_path_quote_promotion(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

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

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    let exclusivity = Duration::from_secs(100);
    let orderbook_config = configs::orderbook::Configuration {
        order_validation: configs::orderbook::order_validation::OrderValidationConfig {
            min_fast_path_exclusivity: Some(exclusivity),
            ..Default::default()
        },
        ..configs::orderbook::Configuration::test_default()
    };
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            orderbook_config,
            solver,
        )
        .await;

    // 1) Fast-path quote request. The opt-in lives on the app-data metadata
    //    (`enableFastPath: true`), not on the quote payload directly.
    tracing::info!("Quoting with enableFastPath");
    let fast_path_app_data = r#"{"metadata":{"enableFastPath":true}}"#;
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
        app_data: OrderCreationAppData::Full {
            full: fast_path_app_data.to_string(),
        },
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote_response
        .id
        .expect("fast-path quote should carry an id");

    // 2) The transient `quotes` row must be tagged with the fast-path
    //    `auction_id`, and that auction id must be present across all
    //    competition tables written at quote time.
    tracing::info!("Verifying competition tables written at quote time");
    let auction_id = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar::<_, Option<i64>>("SELECT auction_id FROM quotes WHERE id = $1")
            .bind(quote_id)
            .fetch_one(db.deref_mut())
            .await
            .unwrap()
            .expect("fast-path quote row should carry an auction_id")
    };

    {
        let mut db = services.db().acquire().await.unwrap();

        let competition_auction: Option<i64> =
            sqlx::query_scalar("SELECT id FROM competition_auctions WHERE id = $1")
                .bind(auction_id)
                .fetch_optional(db.deref_mut())
                .await
                .unwrap();
        assert_eq!(
            competition_auction,
            Some(auction_id),
            "competition_auctions row should exist for the fast-path auction"
        );

        let proposed_solutions: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM proposed_solutions WHERE auction_id = $1")
                .bind(auction_id)
                .fetch_one(db.deref_mut())
                .await
                .unwrap();
        assert!(
            proposed_solutions > 0,
            "expected at least one proposed_solutions row for the fast-path auction"
        );

        // Before the order is placed the placeholder uid (56 zero bytes) stands
        // in for the yet-unknown user order.
        let placeholder: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM proposed_trade_executions WHERE auction_id = $1 AND order_uid = \
             $2",
        )
        .bind(auction_id)
        .bind(ByteArray([0u8; 56]))
        .fetch_one(db.deref_mut())
        .await
        .unwrap();
        assert!(
            placeholder > 0,
            "expected placeholder proposed_trade_executions row before order placement"
        );
    }

    // 3) Place the order referencing this quote.
    tracing::info!("Placing order with the fast-path quote_id");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount: quote_sell_amount,
        buy_token: *token.address(),
        buy_amount: quote_response.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full {
            full: fast_path_app_data.to_string(),
        },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let order_uid = services.create_order(&order).await.unwrap();

    // 4) The promotion must:
    //    - drop the transient `quotes` row (single source of truth guarantee),
    //    - insert `order_quotes` with the same `auction_id`,
    //    - rewrite the placeholder in `proposed_trade_executions` to the real
    //      `order_uid` (and leave no placeholder behind).
    tracing::info!("Verifying quote promotion + competition patch");
    let mut db = services.db().acquire().await.unwrap();

    let quotes_row: Option<i64> = sqlx::query_scalar("SELECT id FROM quotes WHERE id = $1")
        .bind(quote_id)
        .fetch_optional(db.deref_mut())
        .await
        .unwrap();
    assert!(
        quotes_row.is_none(),
        "transient quote row should have been deleted when the order was placed"
    );

    let order_quotes_auction_id: Option<i64> =
        sqlx::query_scalar("SELECT auction_id FROM order_quotes WHERE order_uid = $1")
            .bind(ByteArray(order_uid.0))
            .fetch_one(db.deref_mut())
            .await
            .unwrap();
    assert_eq!(
        order_quotes_auction_id,
        Some(auction_id),
        "order_quotes should carry the promoted auction_id"
    );

    let patched: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proposed_trade_executions WHERE auction_id = $1 AND order_uid = $2",
    )
    .bind(auction_id)
    .bind(ByteArray(order_uid.0))
    .fetch_one(db.deref_mut())
    .await
    .unwrap();
    assert!(
        patched > 0,
        "proposed_trade_executions should reference the real order_uid after placement"
    );

    let leftover_placeholder: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proposed_trade_executions WHERE auction_id = $1 AND order_uid = $2",
    )
    .bind(auction_id)
    .bind(ByteArray([0u8; 56]))
    .fetch_one(db.deref_mut())
    .await
    .unwrap();
    assert_eq!(
        leftover_placeholder, 0,
        "placeholder proposed_trade_executions row should have been overwritten"
    );

    // Log the winning trade execution against the amounts the user actually
    // signed. Currently the executed amounts on the promoted row are still the
    // ones captured at quote time; this makes the drift (if any) visible so a
    // future change that patches them can be spotted from the test output.
    let (winning_sell, winning_buy): (bigdecimal::BigDecimal, bigdecimal::BigDecimal) =
        sqlx::query_as(
            "SELECT pte.executed_sell, pte.executed_buy
             FROM proposed_trade_executions pte
             JOIN proposed_solutions ps
               ON ps.auction_id = pte.auction_id AND ps.uid = pte.solution_uid
             WHERE pte.auction_id = $1
               AND pte.order_uid = $2
               AND ps.is_winner = TRUE",
        )
        .bind(auction_id)
        .bind(ByteArray(order_uid.0))
        .fetch_one(db.deref_mut())
        .await
        .unwrap();
    let signed = order.data();
    tracing::info!(
        %winning_sell,
        %winning_buy,
        signed_sell = %signed.sell_amount,
        signed_buy = %signed.buy_amount,
        "winning proposed_trade_execution vs. signed order amounts"
    );
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_ethflow_promotion() {
    run_test(fast_path_ethflow_promotion).await;
}

/// Same invariants as `fast_path_quote_promotion`, but the order is placed
/// on-chain via the ethflow contract instead of `POST /orders`. Verifies
/// that the autopilot's onchain-event ingestion path also promotes the
/// transient quote and patches the competition placeholder.
async fn fast_path_ethflow_promotion(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(2u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    let exclusivity = Duration::from_secs(100);
    let orderbook_config = configs::orderbook::Configuration {
        order_validation: configs::orderbook::order_validation::OrderValidationConfig {
            min_fast_path_exclusivity: Some(exclusivity),
            ..Default::default()
        },
        ..configs::orderbook::Configuration::test_default()
    };
    services
        .start_protocol_with_args(
            configs::autopilot::Configuration::test("test_solver", solver.address()),
            orderbook_config,
            solver,
        )
        .await;

    // 1) Register the app-data JSON that carries the fast-path opt-in and grab
    //    the returned hash — that's what the ethflow contract will emit
    //    on-chain.
    tracing::info!("Registering fast-path app data");
    let fast_path_app_data = r#"{"metadata":{"enableFastPath":true}}"#;
    let app_data_hex = services
        .put_app_data(None, fast_path_app_data)
        .await
        .unwrap();
    let app_data_hash = AppDataHash(
        const_hex::decode(&app_data_hex[2..])
            .unwrap()
            .try_into()
            .unwrap(),
    );

    // 2) Fast-path quote for the future ethflow order. Ethflow orders sign via
    //    EIP-1271 (owner is the ethflow contract), so the quote must be
    //    requested with that signing scheme.
    tracing::info!("Quoting with enableFastPath");
    let sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        receiver: Some(trader.address()),
        validity: Validity::For(3600),
        app_data: OrderCreationAppData::Hash {
            hash: app_data_hash,
        },
        signing_scheme: QuoteSigningScheme::Eip1271 {
            onchain_order: true,
            verification_gas_limit: 0,
        },
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        price_quality: PriceQuality::Optimal,
        ..Default::default()
    };
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote_response
        .id
        .expect("fast-path quote should carry an id");

    // 3) Same competition-tables invariant as the API-based flow.
    tracing::info!("Verifying competition tables written at quote time");
    let auction_id = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar::<_, Option<i64>>("SELECT auction_id FROM quotes WHERE id = $1")
            .bind(quote_id)
            .fetch_one(db.deref_mut())
            .await
            .unwrap()
            .expect("fast-path quote row should carry an auction_id")
    };
    {
        let mut db = services.db().acquire().await.unwrap();
        let placeholder: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM proposed_trade_executions WHERE auction_id = $1 AND order_uid = \
             $2",
        )
        .bind(auction_id)
        .bind(ByteArray([0u8; 56]))
        .fetch_one(db.deref_mut())
        .await
        .unwrap();
        assert!(
            placeholder > 0,
            "expected placeholder proposed_trade_executions row before ethflow order placement"
        );
    }

    // 4) Place the ethflow order on-chain and wait for the autopilot to index
    //    it. Ethflow orders don't reach the DB via `POST /orders` — they show
    //    up when the autopilot picks up the `OrderPlacement` event.
    tracing::info!("Placing ethflow order on-chain");
    let valid_to = chrono::offset::Utc::now().timestamp() as u32 + 3600;
    let ethflow_order =
        ExtendedEthFlowOrder::from_quote(&quote_response, valid_to).include_slippage_bps(300);
    let ethflow_contract = onchain.contracts().ethflows.first().unwrap();
    ethflow_order
        .mine_order_creation(trader.address(), ethflow_contract)
        .await;

    tracing::info!("Waiting for autopilot to index the ethflow order");
    let order_uid = ethflow_order
        .uid(onchain.contracts(), ethflow_contract)
        .await;
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services.get_order(&order_uid).await.is_ok()
    })
    .await
    .unwrap();

    // 5) The onchain-event ingestion path must apply the same promotion the
    //    orderbook does for API orders.
    tracing::info!("Verifying quote promotion + competition patch for ethflow");
    let mut db = services.db().acquire().await.unwrap();

    let quotes_row: Option<i64> = sqlx::query_scalar("SELECT id FROM quotes WHERE id = $1")
        .bind(quote_id)
        .fetch_optional(db.deref_mut())
        .await
        .unwrap();
    assert!(
        quotes_row.is_none(),
        "transient quote row should have been deleted when the ethflow order was indexed"
    );

    let order_quotes_auction_id: Option<i64> =
        sqlx::query_scalar("SELECT auction_id FROM order_quotes WHERE order_uid = $1")
            .bind(ByteArray(order_uid.0))
            .fetch_one(db.deref_mut())
            .await
            .unwrap();
    assert_eq!(
        order_quotes_auction_id,
        Some(auction_id),
        "order_quotes should carry the promoted auction_id for ethflow orders"
    );

    let patched: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proposed_trade_executions WHERE auction_id = $1 AND order_uid = $2",
    )
    .bind(auction_id)
    .bind(ByteArray(order_uid.0))
    .fetch_one(db.deref_mut())
    .await
    .unwrap();
    assert!(
        patched > 0,
        "proposed_trade_executions should reference the real ethflow order_uid after placement"
    );

    let leftover_placeholder: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proposed_trade_executions WHERE auction_id = $1 AND order_uid = $2",
    )
    .bind(auction_id)
    .bind(ByteArray([0u8; 56]))
    .fetch_one(db.deref_mut())
    .await
    .unwrap();
    assert_eq!(
        leftover_placeholder, 0,
        "placeholder proposed_trade_executions row should have been overwritten"
    );
}
