use {
    database::byte_array::ByteArray,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    shared::web3::Web3,
    std::ops::DerefMut,
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
    services.start_protocol(solver.clone()).await;

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

    // 2) The transient `quotes` row must be tagged with the fast-path `auction_id`,
    //    and that auction id must be present across all competition tables written
    //    at quote time.
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
}
