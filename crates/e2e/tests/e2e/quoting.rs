use {
    e2e::setup::{colocation::SolverEngine, mock::Mock, *},
    ethrpc::alloy::CallBuilderExt,
    futures::FutureExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    serde_json::json,
    shared::{fee_factor::FeeFactor, web3::Web3},
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

#[tokio::test]
#[ignore]
async fn local_node_test() {
    run_test(test).await;
}

#[tokio::test]
#[ignore]
async fn local_node_uses_stale_liquidity() {
    run_test(uses_stale_liquidity).await;
}

#[tokio::test]
#[ignore]
async fn local_node_quote_timeout() {
    run_test(quote_timeout).await;
}

#[tokio::test]
#[ignore]
async fn local_node_volume_fee() {
    run_test(volume_fee).await;
}

// Test that quoting works as expected, specifically, that we can quote for a
// token pair and additional gas from ERC-1271 and hooks are included in the
// quoted fee amount.
async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

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
    let (_config_file, config_arg) = autopilot::config::Configuration::default().to_cli_args();
    // Start API with 0.02% (2 bps) volume fee
    let (_ob_config_file, ob_config_arg) = orderbook::config::Configuration {
        volume_fee: Some(orderbook::config::VolumeFeeConfig {
            factor: Some(FeeFactor::new(0.0002)),
            // Set a far future effective timestamp to ensure the fee is not applied
            effective_from_timestamp: Some("2099-01-01T10:00:00Z".parse().unwrap()),
        }),
        ..Default::default()
    }
    .to_cli_args();
    let args = ExtraServiceArgs {
        api: vec![ob_config_arg],
        autopilot: vec![config_arg],
    };
    services.start_protocol_with_args(args, solver).await;

    tracing::info!("Quoting order");
    let request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(1u64.eth()).unwrap(),
            },
        },
        ..Default::default()
    };

    let with_eip1271 = services
        .submit_quote(&OrderQuoteRequest {
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: 50_000,
            },
            ..request.clone()
        })
        .await
        .unwrap();

    let with_hooks = services
        .submit_quote(&OrderQuoteRequest {
            app_data: OrderCreationAppData::Full {
                full: serde_json::to_string(&json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                            "post": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                        },
                    },
                }))
                .unwrap(),
            },
            ..request.clone()
        })
        .await
        .unwrap();

    let with_both = services
        .submit_quote(&OrderQuoteRequest {
            signing_scheme: QuoteSigningScheme::Eip1271 {
                onchain_order: false,
                verification_gas_limit: 50_000,
            },
            app_data: OrderCreationAppData::Full {
                full: serde_json::to_string(&json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                            "post": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "5000",
                                },
                            ],
                        },
                    },
                }))
                .unwrap(),
            },
            ..request.clone()
        })
        .await
        .unwrap();

    let base = services.submit_quote(&request).await.unwrap();

    tracing::debug!(
        ?with_eip1271,
        ?with_hooks,
        ?with_both,
        ?base,
        "Computed quotes."
    );

    assert!(base.quote.fee_amount < with_eip1271.quote.fee_amount);
    assert!(base.quote.fee_amount < with_hooks.quote.fee_amount);

    assert!(with_both.quote.fee_amount > with_eip1271.quote.fee_amount);
    assert!(with_both.quote.fee_amount > with_hooks.quote.fee_amount);

    // TODO: test verified quotes, requires state overrides support.
}

async fn uses_stale_liquidity(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    onchain
        .contracts()
        .weth
        .approve(onchain.contracts().allowance, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();
    onchain
        .contracts()
        .weth
        .deposit()
        .from(trader.address())
        .value(1u64.eth())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let quote = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::new(1u64.eth()).unwrap(),
            },
        },
        ..Default::default()
    };

    tracing::info!("performining initial quote");
    let first = services.submit_quote(&quote).await.unwrap();

    // Now, we want to manually unbalance the pools and assert that the quote
    // doesn't change (as the price estimation will use stale pricing data).
    onchain
        .mint_token_to_weth_uni_v2_pool(&token, 1_000u64.eth())
        .await;

    tracing::info!("performining second quote, which should match first");
    let second = services.submit_quote(&quote).await.unwrap();
    assert_eq!(first.quote.buy_amount, second.quote.buy_amount);

    tracing::info!("waiting for liquidity state to update");
    wait_for_condition(TIMEOUT, || async {
        // Mint blocks until we evict the cached liquidty and fetch the new state.
        onchain.mint_block().await;
        let Ok(next) = services.submit_quote(&quote).await else {
            return false;
        };
        next.quote.buy_amount != first.quote.buy_amount
    })
    .await
    .unwrap();
}

/// Tests that the user can provide a timeout with their quote
/// which gets respected.
async fn quote_timeout(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;
    let [sell_token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    let mock_solver = Mock::new().await;

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            SolverEngine {
                name: "test_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![*sell_token.address()],
                merge_solutions: true,
                haircut_bps: 0,
            },
            SolverEngine {
                name: "test_quoter".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![*sell_token.address()],
                merge_solutions: true,
                haircut_bps: 0,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    /// The default and maximum quote timeout enforced by the backend.
    /// (configurable but always 500ms in e2e tests)
    const MAX_QUOTE_TIME_MS: u64 = 500;

    let (_ob_config_file, ob_config_arg) = orderbook::config::Configuration {
        native_price_estimation: orderbook::config::native_price::NativePriceConfig {
            estimators: shared::price_estimation::NativePriceEstimators::new(vec![vec![
                shared::price_estimation::NativePriceEstimator::driver(
                    "test_quoter".to_string(),
                    "http://localhost:11088/test_solver".parse().unwrap(),
                ),
            ]]),
            ..Default::default()
        },
        ..Default::default()
    }
    .to_cli_args();
    services
        .start_api(vec![
            ob_config_arg,
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_quoter".to_string(),
            format!("--quote-timeout={MAX_QUOTE_TIME_MS}ms"),
        ])
        .await;

    mock_solver.configure_solution_async(Arc::new(|| {
        async {
            // make the solver always exceeds the maximum allowed timeout
            // (by default 500ms in e2e tests)
            tokio::time::sleep(Duration::from_millis(MAX_QUOTE_TIME_MS + 300)).await;
            // we only care about timeout management so no need to return
            // a working solution
            None
        }
        .boxed()
    }));

    let quote_request = |timeout| OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *sell_token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(1u64.eth()).unwrap(),
            },
        },
        timeout,
        ..Default::default()
    };

    let assert_within_variance = |start_timestamp: Instant, target| {
        const VARIANCE: u64 = 100; // small buffer to allow for variance in the test
        const HTTP_BUFFER: u64 = 100;
        let min = target - HTTP_BUFFER;
        let max = min + VARIANCE;
        let elapsed = start_timestamp.elapsed().as_millis() as u64;
        tracing::debug!(target, actual = ?elapsed, "finished request");
        assert!((min..max).contains(&elapsed));
    };

    // native token price requests are also capped to the max timeout
    let start = std::time::Instant::now();
    let res = services.get_native_price(sell_token.address()).await;
    assert!(res.unwrap_err().1.contains("NoLiquidity"));
    assert_within_variance(start, MAX_QUOTE_TIME_MS);

    // not providing a timeout uses the backend's default timeout (500ms)
    let start = std::time::Instant::now();
    let res = services.submit_quote(&quote_request(None)).await;
    assert!(res.unwrap_err().1.contains("NoLiquidity"));
    assert_within_variance(start, MAX_QUOTE_TIME_MS);

    // timeouts below the max timeout get enforced correctly
    let start = std::time::Instant::now();
    let res = services
        .submit_quote(&quote_request(Some(Duration::from_millis(300))))
        .await;
    assert!(res.unwrap_err().1.contains("NoLiquidity"));
    assert_within_variance(start, 300);

    // user provided timeouts get capped at the backend's max timeout (500ms)
    let start = std::time::Instant::now();
    let res = services
        .submit_quote(&quote_request(Some(Duration::from_millis(
            MAX_QUOTE_TIME_MS * 2,
        ))))
        .await;
    assert!(res.unwrap_err().1.contains("NoLiquidity"));
    assert_within_variance(start, MAX_QUOTE_TIME_MS);

    // set up trader to pass balance checks during order creation
    sell_token.mint(trader.address(), 1u64.eth()).await;

    sell_token
        .approve(onchain.contracts().allowance, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    let order = OrderCreation {
        sell_token: *sell_token.address(),
        sell_amount: 1u64.eth(),
        buy_token: Default::default(),
        buy_amount: 1u64.eth(),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    // order creation requests always use the default quote time
    // to maximize the chance for the request to succeed because
    // we return an error if we can't get a quote
    let start = std::time::Instant::now();
    let res = services.create_order(&order).await;
    assert!(res.unwrap_err().1.contains("NoLiquidity"));
    assert_within_variance(start, MAX_QUOTE_TIME_MS);
}

/// Test that volume fees are correctly applied to quotes.
async fn volume_fee(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(10u64.eth()).await;
    let [token, override_token] = onchain
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

    tracing::info!("Starting services with volume fee.");
    let services = Services::new(&onchain).await;
    let (_config_file, config_arg) = autopilot::config::Configuration::default().to_cli_args();
    // Start API with 0.02% (2 bps) default volume fee
    // Bucket override: WETH<->override_token pair gets 5 bps (both tokens must be
    // in bucket)
    let (_ob_config_file, ob_config_arg) = orderbook::config::Configuration {
        volume_fee: Some(orderbook::config::VolumeFeeConfig {
            factor: Some(FeeFactor::new(0.0002)),
            // Set a past effective timestamp to ensure the fee is applied
            effective_from_timestamp: Some("2000-01-01T10:00:00Z".parse().unwrap()),
        }),
        ..Default::default()
    }
    .to_cli_args();
    let args = ExtraServiceArgs {
        api: vec![
            ob_config_arg,
            format!(
                "--volume-fee-bucket-overrides=0.0005:{};{}",
                onchain.contracts().weth.address(),
                override_token.address()
            ),
        ],
        autopilot: vec![config_arg],
    };
    services.start_protocol_with_args(args, solver).await;

    tracing::info!("Testing SELL quote with volume fee");
    let sell_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(1u64.eth()).unwrap(),
            },
        },
        ..Default::default()
    };

    let sell_quote = services.submit_quote(&sell_request).await.unwrap();

    // Verify protocol fee fields are present
    assert!(sell_quote.protocol_fee_bps.is_some());
    assert_eq!(sell_quote.protocol_fee_bps.as_ref().unwrap(), "2");

    tracing::info!("Testing BUY quote with volume fee");
    let buy_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Buy {
            buy_amount_after_fee: NonZeroU256::try_from(1u64.eth()).unwrap(),
        },
        ..Default::default()
    };

    let buy_quote = services.submit_quote(&buy_request).await.unwrap();

    // Verify protocol fee fields are present
    assert!(buy_quote.protocol_fee_bps.is_some());
    assert_eq!(buy_quote.protocol_fee_bps.as_ref().unwrap(), "2");

    // Test bucket override: override_token should get 5 bps instead of 2 bps
    tracing::info!("Testing quote with bucket override (5 bps)");
    let override_request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *override_token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(1u64.eth()).unwrap(),
            },
        },
        ..Default::default()
    };

    let override_quote = services.submit_quote(&override_request).await.unwrap();

    // Verify override token gets 5 bps (from bucket override) instead of 2 bps
    // (default)
    assert!(override_quote.protocol_fee_bps.is_some());
    assert_eq!(
        override_quote.protocol_fee_bps.as_ref().unwrap(),
        "5",
        "Bucket override should apply 5 bps, not default 2 bps"
    );
}
