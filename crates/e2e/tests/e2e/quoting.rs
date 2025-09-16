use {
    e2e::{
        setup::{colocation::SolverEngine, eth, mock::Mock, *},
        tx,
        tx_value,
    },
    ethrpc::alloy::{
        CallBuilderExt,
        conversions::{IntoAlloy, IntoLegacy},
    },
    futures::FutureExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, QuoteSigningScheme, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::nonzero::U256 as NonZeroU256,
    secp256k1::SecretKey,
    serde_json::json,
    shared::ethrpc::Web3,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
    web3::signing::SecretKeyRef,
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

// Test that quoting works as expected, specifically, that we can quote for a
// token pair and additional gas from ERC-1271 and hooks are included in the
// quoted fee amount.
async fn test(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(10)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(3))
    );
    tx_value!(
        trader.account(),
        to_wei(3),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    tracing::info!("Quoting order");
    let request = OrderQuoteRequest {
        from: trader.address(),
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address().into_legacy(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(to_wei(1)).unwrap(),
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

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(2)).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tx!(
        trader.account(),
        onchain
            .contracts()
            .weth
            .approve(onchain.contracts().allowance, to_wei(1))
    );
    tx_value!(
        trader.account(),
        to_wei(1),
        onchain.contracts().weth.deposit()
    );

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    let quote = OrderQuoteRequest {
        from: trader.address(),
        sell_token: onchain.contracts().weth.address(),
        buy_token: token.address().into_legacy(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::AfterFee {
                value: NonZeroU256::new(to_wei(1)).unwrap(),
            },
        },
        ..Default::default()
    };

    tracing::info!("performining initial quote");
    let first = services.submit_quote(&quote).await.unwrap();

    // Now, we want to manually unbalance the pools and assert that the quote
    // doesn't change (as the price estimation will use stale pricing data).
    onchain
        .mint_token_to_weth_uni_v2_pool(&token, to_wei(1_000))
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

    let [solver] = onchain.make_solvers(to_wei(10)).await;
    let [trader] = onchain.make_accounts(to_wei(2)).await;
    let [sell_token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(to_wei(1_000), to_wei(1_000))
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    let mock_solver = Mock::default();

    // Start system
    colocation::start_driver(
        onchain.contracts(),
        vec![
            SolverEngine {
                name: "test_solver".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![sell_token.address().into_legacy()],
                merge_solutions: true,
            },
            SolverEngine {
                name: "test_quoter".into(),
                account: solver.clone(),
                endpoint: mock_solver.url.clone(),
                base_tokens: vec![sell_token.address().into_legacy()],
                merge_solutions: true,
            },
        ],
        colocation::LiquidityProvider::UniswapV2,
        false,
    );

    /// The default and maximum quote timeout enforced by the backend.
    /// (configurable but always 500ms in e2e tests)
    const MAX_QUOTE_TIME_MS: u64 = 500;

    services
        .start_api(vec![
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
        sell_token: onchain.contracts().weth.address(),
        buy_token: sell_token.address().into_legacy(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: NonZeroU256::try_from(to_wei(1)).unwrap(),
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
    let res = services
        .get_native_price(&sell_token.address().into_legacy())
        .await;
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
    sell_token.mint(trader.address(), to_wei(1)).await;

    sell_token
        .approve(onchain.contracts().allowance.into_alloy(), eth(1))
        .from(trader.address().into_alloy())
        .send_and_watch()
        .await
        .unwrap();

    let order = OrderCreation {
        sell_token: sell_token.address().into_legacy(),
        sell_amount: to_wei(1),
        buy_token: Default::default(),
        buy_amount: to_wei(1),
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        partially_fillable: true,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        SecretKeyRef::from(&SecretKey::from_slice(trader.private_key()).unwrap()),
    );

    // order creation requests always use the default quote time
    // to maximize the chance for the request to succeed because
    // we return an error if we can't get a quote
    let start = std::time::Instant::now();
    let res = services.create_order(&order).await;
    assert!(res.unwrap_err().1.contains("NoLiquidity"));
    assert_within_variance(start, MAX_QUOTE_TIME_MS);
}
