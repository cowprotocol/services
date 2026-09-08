use {
    ::alloy::primitives::{Address, U256},
    configs::{
        autopilot::{
            Configuration as AutopilotConfiguration,
            fee_policy::{
                FeePoliciesConfig,
                FeePolicy as ConfigFeePolicy,
                FeePolicyKind as ConfigFeePolicyKind,
                FeePolicyOrderClass as ConfigFeePolicyOrderClass,
            },
            solver::Solver,
        },
        test_util::TestDefault,
    },
    database::byte_array::ByteArray,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        fee_policy::FeePolicy as TradeFeePolicy,
        order::{OrderCreation, OrderCreationAppData, OrderKind, OrderStatus},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    serde_json::json,
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

#[tokio::test]
#[ignore]
async fn local_node_fast_path_volume_fees_captured() {
    run_test(fast_path_volume_fees_captured).await;
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

/// Configures a protocol volume fee via the autopilot config and a partner
/// volume fee via app-data, and verifies that both are recorded against the
/// fast-path order and that every bid's `executed_sell`/`executed_buy` reflects
/// the compounded fee reduction.
async fn fast_path_volume_fees_captured(web3: Web3) {
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

    // 1% protocol volume fee applied to any order class.
    let protocol_volume_factor: f64 = 0.01;
    // 2% partner volume fee (200 bps).
    let partner_volume_bps: u64 = 200;
    let partner_recipient = Address::repeat_byte(0xb0);
    let exclusivity = Duration::from_secs(300);

    let orderbook_config = configs::orderbook::Configuration {
        order_validation: configs::orderbook::order_validation::OrderValidationConfig {
            min_fast_path_exclusivity: Some(exclusivity),
            ..Default::default()
        },
        ..configs::orderbook::Configuration::test_default()
    };
    let autopilot_config = AutopilotConfiguration {
        drivers: vec![Solver::test("test_solver", solver.address())],
        fee_policies: FeePoliciesConfig {
            policies: vec![ConfigFeePolicy {
                kind: ConfigFeePolicyKind::Volume {
                    factor: protocol_volume_factor.try_into().unwrap(),
                },
                order_class: ConfigFeePolicyOrderClass::Any,
            }],
            // Room for the partner factor (2%).
            max_partner_fee: 0.05.try_into().unwrap(),
            ..Default::default()
        },
        ..AutopilotConfiguration::test_no_drivers()
    };
    services
        .start_protocol_with_args(autopilot_config, orderbook_config, solver)
        .await;

    let app_data = json!({
        "version": "1.1.0",
        "metadata": {
            "enableFastPath": true,
            "partnerFee": {
                "bps": partner_volume_bps,
                "recipient": partner_recipient,
            }
        }
    })
    .to_string();

    tracing::info!("Quoting with enableFastPath and a partner fee.");
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

    // Sign a buy amount comfortably below the quote so the fee-reduced
    // `executed_buy` still clears the on-chain limit-price check.
    let signed_buy = quote.quote.buy_amount * U256::from(90u64) / U256::from(100u64);
    tracing::info!("Placing the fast-path order.");
    let order = OrderCreation {
        quote_id: Some(quote_id),
        sell_token: *onchain.contracts().weth.address(),
        sell_amount,
        buy_token: *token.address(),
        buy_amount: signed_buy,
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

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();

    // The /trades API rebuilds the fees from `order_execution` (written by
    // the settlement observer once the tx is mined). The observer runs
    // slightly after the trade event, so wait for both fee entries to appear.
    tracing::info!("Waiting for /trades to report both volume fees.");
    wait_for_condition(TIMEOUT, || async {
        services.get_trades(&uid).await.is_ok_and(|trades| {
            trades
                .first()
                .is_some_and(|t| t.executed_protocol_fees.len() == 2)
        })
    })
    .await
    .unwrap();
    let trades = services.get_trades(&uid).await.unwrap();
    assert_eq!(
        trades.len(),
        1,
        "expected one trade for the fast-path order"
    );
    let trade = &trades[0];
    assert_eq!(
        trade.executed_protocol_fees.len(),
        2,
        "expected two Volume fee entries (protocol + partner), got {:?}",
        trade.executed_protocol_fees
    );

    let buy_token = *token.address();
    let expected_partner_factor = partner_volume_bps as f64 / 10_000.0;
    let expected_factors = [protocol_volume_factor, expected_partner_factor];
    for (fee, expected_factor) in trade.executed_protocol_fees.iter().zip(expected_factors) {
        assert_eq!(
            fee.token, buy_token,
            "fees on a sell order are taken from the buy token"
        );
        assert!(!fee.amount.is_zero(), "fee amount should be positive");
        match fee.policy {
            TradeFeePolicy::Volume { factor } => assert!(
                (factor - expected_factor).abs() < 1e-9,
                "unexpected volume factor {factor} (expected {expected_factor})"
            ),
            ref other => panic!("fast-path fees should all be Volume, got {other:?}"),
        }
    }

    // The on-chain buy_amount should be strictly smaller than what the API
    // quoted — a sanity check that fees actually shrunk the fill, and not
    // just that the fee rows exist.
    let executed_buy = number::conversions::big_uint_to_u256(&trade.buy_amount)
        .expect("trade buy amount fits in U256");
    assert!(
        executed_buy < quote.quote.buy_amount,
        "executed buy {executed_buy} should be below the raw quote {} once fees are taken",
        quote.quote.buy_amount
    );
}
