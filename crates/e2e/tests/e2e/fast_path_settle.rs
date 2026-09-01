use {
    configs::test_util::TestDefault,
    database::byte_array::ByteArray,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    shared::web3::Web3,
    std::{ops::DerefMut, time::Duration},
};

#[tokio::test]
#[ignore]
async fn local_node_fast_path_settle() {
    run_test(fast_path_settle).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fast_path_regular_auction_fallback() {
    run_test(fast_path_regular_auction_fallback).await;
}

async fn fast_path_settle(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    // A long fast-path exclusivity so only the fast path can settle the order
    // within the test window.
    let exclusivity = Duration::from_secs(300);
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

    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();

    tracing::info!("Quoting with enableFastPath.");
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(sell_amount).unwrap(),
            },
        },
        app_data: OrderCreationAppData::Full {
            full: app_data.clone(),
        },
        ..Default::default()
    };
    let quote = services.submit_quote(&quote_request).await.unwrap();
    let quote_id = quote.id.expect("fast-path quote should carry an id");

    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: quote.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    let valid_from = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar::<_, Option<i64>>("SELECT valid_from FROM orders WHERE uid = $1")
            .bind(ByteArray(uid.0))
            .fetch_one(db.deref_mut())
            .await
            .unwrap()
            .expect("fast-path order has a valid_from") as u32
    };
    let expected = model::time::now_in_epoch_seconds() + exclusivity.as_secs() as u32;
    assert!(
        valid_from.abs_diff(expected) <= 2,
        "valid_from {valid_from} should be ~{expected} (now + exclusivity)"
    );

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    assert!(
        model::time::now_in_epoch_seconds() < valid_from,
        "order settled after the exclusivity window; can't attribute it to the fast path"
    );
    // The order only appears in its own quote (fast-path) auction, never a
    // regular batch auction during the exclusive window.
    let regular_auctions: Vec<i64> = {
        let mut db = services.db().acquire().await.unwrap();
        let quote_auction: Option<i64> =
            sqlx::query_scalar("SELECT auction_id FROM order_quotes WHERE order_uid = $1")
                .bind(ByteArray(uid.0))
                .fetch_one(db.deref_mut())
                .await
                .unwrap();
        let quote_auction = quote_auction.expect("fast-path order has a quote auction");
        sqlx::query_scalar(
            "SELECT id FROM competition_auctions WHERE order_uids @> ARRAY[$1::bytea] AND id != $2",
        )
        .bind(ByteArray(uid.0))
        .bind(quote_auction)
        .fetch_all(db.deref_mut())
        .await
        .unwrap()
    };
    assert!(
        regular_auctions.is_empty(),
        "fast-path order must not appear in a regular auction: {regular_auctions:?}"
    );
}

/// When a fast-path order's exclusive window elapses without the fast path
/// settling it, the regular auction settles it once `valid_from` passes. The
/// order is quoted without `enableFastPath`, so no fast-path solution is cached
/// and the settler has nothing to submit — standing in for a solver that held
/// the exclusive window but never settled.
async fn fast_path_regular_auction_fallback(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    let sell_amount = 1u64.eth();
    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, sell_amount)
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(sell_amount)
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    // A short window so the regular auction picks the order up soon after it
    // elapses, within the test timeout.
    let exclusivity = Duration::from_secs(5);
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

    // A plain quote leaves no cached fast-path solution.
    tracing::info!("Quoting without enableFastPath.");
    let quote_request = OrderQuoteRequest {
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
    let quote = services.submit_quote(&quote_request).await.unwrap();

    // The order still requests the fast path, so the orderbook holds it out of
    // the auction until `valid_from`.
    tracing::info!("Placing the fast-path order.");
    let app_data = r#"{"metadata":{"enableFastPath":true}}"#.to_string();
    let order = OrderCreation {
        quote_id: quote.id,
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: quote.quote.buy_amount,
        valid_to: model::time::now_in_epoch_seconds() + 3600,
        kind: OrderKind::Sell,
        app_data: OrderCreationAppData::Full { full: app_data },
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );
    let uid = services.create_order(&order).await.unwrap();

    // Held out: not settled early by the fast path.
    assert_eq!(
        services.get_order(&uid).await.unwrap().metadata.status,
        OrderStatus::Open
    );
    let valid_from = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar::<_, Option<i64>>("SELECT valid_from FROM orders WHERE uid = $1")
            .bind(ByteArray(uid.0))
            .fetch_one(db.deref_mut())
            .await
            .unwrap()
            .expect("fast-path order has a valid_from") as u32
    };
    assert!(
        valid_from > model::time::now_in_epoch_seconds(),
        "valid_from {valid_from} should be in the future (order held out)"
    );

    tracing::info!("Waiting for the regular-auction settlement.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // Settled only after the exclusive window elapsed.
    assert!(
        model::time::now_in_epoch_seconds() >= valid_from,
        "order settled before the exclusivity window elapsed"
    );
    // A plain quote writes no competition auction, so any auction carrying the
    // order is a regular one — proof it settled via the regular auction.
    let regular_auctions: Vec<i64> = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar(
            "SELECT id FROM competition_auctions WHERE order_uids @> ARRAY[$1::bytea]",
        )
        .bind(ByteArray(uid.0))
        .fetch_all(db.deref_mut())
        .await
        .unwrap()
    };
    assert!(
        !regular_auctions.is_empty(),
        "fast-path order should have settled via a regular auction"
    );
}
