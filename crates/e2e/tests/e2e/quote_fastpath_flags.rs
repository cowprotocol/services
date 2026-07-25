use {
    configs::{
        order_quoting::{ExternalSolver, OrderQuoting},
        shared::SharedConfig,
        test_util::TestDefault,
    },
    e2e::setup::{OnchainComponents, Services, run_test},
    eth_domain_types::Address,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    reqwest::StatusCode,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn local_node_quote_fastpath_flags_rejected() {
    run_test(quote_fastpath_flags_rejected).await;
}

/// Verifies that the orderbook rejects quotes and orders that use the
/// not-yet-supported `fast_path` quote flag or `validFrom`/`enableFastPath`
/// app-data fields.
async fn quote_fastpath_flags_rejected(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let services = Services::new(&onchain).await;
    services
        .start_api(configs::orderbook::Configuration {
            order_quoting: OrderQuoting::test_with_drivers(vec![ExternalSolver::new(
                "test_quoter",
                "http://localhost:11088/test_solver",
            )]),
            shared: SharedConfig {
                gas_estimators: vec![TestDefault::test_default()],
                ..Default::default()
            },
            ..configs::orderbook::Configuration::test_default()
        })
        .await;

    // Distinct, non-native addresses so partial_validate passes before the
    // fast-path check fires.
    let sell_token = Address::with_last_byte(2);
    let buy_token = Address::with_last_byte(3);

    let base_quote = || OrderQuoteRequest {
        from: trader.address(),
        sell_token,
        buy_token,
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: 1u64.eth().try_into().unwrap(),
            },
        },
        ..Default::default()
    };

    // --- quote: enableFastPath in app data ---
    let err = services
        .submit_quote(&OrderQuoteRequest {
            app_data: OrderCreationAppData::Full {
                full: r#"{"metadata":{"enableFastPath":true}}"#.to_string(),
            },
            ..base_quote()
        })
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("enableFastPath"),
        "error body should mention enableFastPath, got: {}",
        err.1
    );

    // --- quote: validFrom in app data ---
    let err = services
        .submit_quote(&OrderQuoteRequest {
            app_data: OrderCreationAppData::Full {
                full: r#"{"metadata":{"validFrom":1700000000}}"#.to_string(),
            },
            ..base_quote()
        })
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("validFrom"),
        "error body should mention validFrom, got: {}",
        err.1
    );

    // For order tests, validate_app_data fires before signature verification so
    // we just need structurally valid (but cryptographically incorrect) orders.
    let valid_to = model::time::now_in_epoch_seconds() + 300;

    let make_order = |app_data_str: &str| -> OrderCreation {
        OrderCreation {
            sell_token,
            sell_amount: 1u64.eth(),
            buy_token,
            buy_amount: 1u64.eth(),
            valid_to,
            kind: OrderKind::Sell,
            app_data: OrderCreationAppData::Full {
                full: app_data_str.to_string(),
            },
            ..Default::default()
        }
        .sign(
            EcdsaSigningScheme::Eip712,
            &onchain.contracts().domain_separator,
            &trader.signer,
        )
    };

    // --- order: enableFastPath in app data ---
    let err = services
        .create_order(&make_order(r#"{"metadata":{"enableFastPath":true}}"#))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("enableFastPath"),
        "error body should mention enableFastPath, got: {}",
        err.1
    );

    // --- order: validFrom in app data ---
    let err = services
        .create_order(&make_order(r#"{"metadata":{"validFrom":1700000000}}"#))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(
        err.1.contains("validFrom"),
        "error body should mention validFrom, got: {}",
        err.1
    );
}
