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
    let orderbook_config = configs::orderbook::Configuration {
        order_validation: configs::orderbook::order_validation::OrderValidationConfig {
            min_fast_path_exclusivity: Duration::from_secs(300),
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

    let valid_from: Option<i64> = {
        let mut db = services.db().acquire().await.unwrap();
        sqlx::query_scalar("SELECT valid_from FROM orders WHERE uid = $1")
            .bind(ByteArray(uid.0))
            .fetch_one(db.deref_mut())
            .await
            .unwrap()
    };
    assert!(
        valid_from.is_some_and(|v| v as u32 > model::time::now_in_epoch_seconds()),
        "fast-path order should have a future valid_from set by the orderbook"
    );

    tracing::info!("Waiting for the fast-path settlement.");
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        services
            .get_order(&uid)
            .await
            .is_ok_and(|order| order.metadata.status == OrderStatus::Fulfilled)
    })
    .await
    .unwrap();
}
