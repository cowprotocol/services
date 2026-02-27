use {
    ::alloy::primitives::U256,
    autopilot::{config::Configuration, shutdown_controller::ShutdownController},
    driver::domain::eth::NonZeroU256,
    e2e::setup::{colocation, wait_for_condition, *},
    ethrpc::alloy::{CallBuilderExt, EvmProviderExt},
    model::{
        order::{OrderCreation, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    shared::web3::Web3,
    std::ops::DerefMut,
};

#[tokio::test]
#[ignore]
async fn local_node_place_order_with_quote_basic() {
    run_test(place_order_with_quote).await;
}

#[tokio::test]
#[ignore]
async fn local_node_disabled_same_sell_and_buy_token_order_feature() {
    run_test(disabled_same_sell_and_buy_token_order_feature).await;
}

#[tokio::test]
#[ignore]
async fn local_node_fallback_native_price_estimator() {
    run_test(fallback_native_price_estimator).await;
}

async fn place_order_with_quote(web3: Web3) {
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

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Quoting");
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
    tracing::debug!(?quote_response);
    assert!(quote_response.id.is_some());
    assert!(quote_response.verified);

    let quote_metadata =
        crate::database::quote_metadata(services.db(), quote_response.id.unwrap()).await;
    assert!(quote_metadata.is_some());
    tracing::debug!(?quote_metadata);

    tracing::info!("Placing order");
    let balance = token.balanceOf(trader.address()).call().await.unwrap();
    assert_eq!(balance, U256::ZERO);
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

    tracing::info!("Order quote verification");
    let order_quote = database::orders::read_quote(
        services.db().acquire().await.unwrap().deref_mut(),
        &database::byte_array::ByteArray(order_uid.0),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(quote_response.verified, order_quote.verified);
    assert_eq!(quote_metadata.unwrap().0, order_quote.metadata);
}

async fn disabled_same_sell_and_buy_token_order_feature(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 10u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 10u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    // Disable auto-mine so we don't accidentally mine a settlement
    web3.provider
        .evm_set_automine(false)
        .await
        .expect("Must be able to disable automine");

    tracing::info!("Quoting");
    let quote_sell_amount = 1u64.eth();
    let quote_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *token.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(quote_sell_amount).unwrap(),
            },
        },
        ..Default::default()
    };
    assert!(
        matches!(services.submit_quote(&quote_request).await, Err((reqwest::StatusCode::BAD_REQUEST, response)) if response.contains("SameBuyAndSellToken"))
    );
}

async fn fallback_native_price_estimator(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 6u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(6u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    colocation::start_driver(
        onchain.contracts(),
        vec![
            colocation::start_baseline_solver(
                "test_solver".into(),
                solver.clone(),
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

    let (manual_shutdown, control) = ShutdownController::new_manual_shutdown();
    let (_autopilot_config_file, cli_arg) =
        Configuration::test("test_solver", solver.address()).to_cli_args();
    let autopilot_handle = services
        .start_autopilot_with_shutdown_controller(
            None,
            vec![
                cli_arg,
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--gas-estimators=http://localhost:11088/gasprice".to_string(),
            ],
            control,
        )
        .await;

    let (_ob_config_file, ob_config_arg) = orderbook::config::Configuration {
        native_price_estimation: orderbook::config::native_price::NativePriceConfig {
            fallback_estimators: Some(shared::price_estimation::NativePriceEstimators::new(vec![
                vec![shared::price_estimation::NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    "http://localhost:11088/test_solver".parse().unwrap(),
                )],
            ])),
            shared: shared::price_estimation::config::native_price::NativePriceConfig {
                cache: shared::price_estimation::config::native_price::CacheConfig {
                    max_age: std::time::Duration::from_secs(2),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();
    services
        .start_api(vec![
            ob_config_arg,
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
            "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        ])
        .await;

    tracing::info!("Quoting with autopilot running");
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
    tracing::debug!(?quote_response);
    assert!(quote_response.id.is_some());

    tracing::info!("Placing order with autopilot running");
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
    services.create_order(&order).await.unwrap();

    tracing::info!("Shutting down autopilot");
    manual_shutdown.shutdown();
    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        autopilot_handle.is_finished()
    })
    .await
    .unwrap();

    // Wait for native price cache to expire (max age = 2s)
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // The FallbackNativePriceEstimator switches to fallback after 3 consecutive
    // ProtocolInternal errors from the primary (forwarder → dead autopilot).
    tracing::info!("Waiting for native price fallback to activate");
    wait_for_condition(TIMEOUT, || async {
        services.get_native_price(token.address()).await.is_ok()
    })
    .await
    .unwrap();

    tracing::info!("Quoting after autopilot shutdown (via fallback)");
    let quote_response = services.submit_quote(&quote_request).await.unwrap();
    tracing::debug!(?quote_response);
    assert!(quote_response.id.is_some());

    tracing::info!("Placing order after autopilot shutdown (via fallback)");
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
    services.create_order(&order).await.unwrap();
}
