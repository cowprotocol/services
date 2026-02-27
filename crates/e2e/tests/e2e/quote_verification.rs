use {
    ::alloy::{
        primitives::{Address, U256, address, map::AddressMap},
        providers::Provider,
    },
    autopilot::config::Configuration,
    bigdecimal::{BigDecimal, Zero},
    e2e::setup::*,
    ethrpc::{Web3, alloy::CallBuilderExt},
    model::{
        interaction::InteractionData,
        order::{BuyTokenDestination, OrderKind, SellTokenSource},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
    },
    number::{nonzero::NonZeroU256, units::EthUnit},
    serde_json::json,
    shared::{
        price_estimation::{
            Estimate, Verification,
            trade_verifier::{
                PriceQuery, TradeVerifier, TradeVerifying,
                balance_overrides::{BalanceOverrides, BalanceOverriding, Strategy},
            },
        },
        trade_finding::{Interaction, LegacyTrade, QuoteExecution, TradeKind},
    },
    std::sync::Arc,
};

#[tokio::test]
#[ignore]
async fn local_node_standard_verified_quote() {
    run_test(standard_verified_quote).await;
}

#[tokio::test]
#[ignore]
async fn forked_node_bypass_verification_for_rfq_quotes() {
    run_forked_test_with_block_number(
        test_bypass_verification_for_rfq_quotes,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_eth_balance() {
    run_test(verified_quote_eth_balance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_for_settlement_contract() {
    run_test(verified_quote_for_settlement_contract).await;
}

#[tokio::test]
#[ignore]
async fn local_node_verified_quote_with_simulated_balance() {
    run_test(verified_quote_with_simulated_balance).await;
}

#[tokio::test]
#[ignore]
async fn local_node_trace_based_balance_detection() {
    run_test(trace_based_balance_detection).await;
}

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_usdt_quote() {
    run_forked_test_with_block_number(
        usdt_quote_verification,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        23112197,
    )
    .await;
}

/// Verified quotes work as expected.
async fn standard_verified_quote(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    token.mint(trader.address(), 1u64.eth()).await;

    token
        .approve(onchain.contracts().allowance, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // quote where the trader has sufficient balance and an approval set.
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *token.address(),
            buy_token: *onchain.contracts().weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// The block number from which we will fetch state for the forked tests.
const FORK_BLOCK_MAINNET: u64 = 23112197;

/// Tests that quotes requesting `tx_origin: 0x0000` bypass the verification
/// because those are currently used by some solvers to provide market maker
/// integrations. Based on an RFQ quote we saw on prod:
/// https://www.tdly.co/shared/simulation/7402de5e-e524-4e24-9af8-50d0a38c105b
async fn test_bypass_verification_for_rfq_quotes(web3: Web3) {
    // This RPC node should support websockets
    let mut url: url::Url = std::env::var("FORK_URL_MAINNET")
        .expect("FORK_URL_MAINNET must be set to run forked tests")
        .parse()
        .unwrap();
    match url.scheme() {
        "http" => url.set_scheme("ws").unwrap(),
        "https" => url.set_scheme("wss").unwrap(),
        _ => unreachable!("unexpected scheme"),
    }
    let block_stream = ethrpc::block_stream::current_block_ws_stream(web3.provider.clone(), url)
        .await
        .unwrap();
    let onchain = OnchainComponents::deployed(web3.clone()).await;

    let verifier = TradeVerifier::new(
        web3.clone(),
        None,
        Arc::new(web3.clone()),
        Arc::new(BalanceOverrides::default()),
        block_stream,
        *onchain.contracts().gp_settlement.address(),
        *onchain.contracts().weth.address(),
        BigDecimal::zero(),
        Default::default(),
    )
    .await
    .unwrap();

    let verify_trade = |tx_origin| {
        let verifier = verifier.clone();
        async move {
            verifier
                .verify(
                    &PriceQuery {
                        sell_token: address!("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"),
                        buy_token: address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                        kind: OrderKind::Sell,
                        in_amount: NonZeroU256::new(U256::from(12)).unwrap(),
                    },
                    &Verification {
                        from: address!("0x73688c2b34bf6c09c125fed02fe92d17a94b897a"),
                        receiver: address!("0x73688c2b34bf6c09c125fed02fe92d17a94b897a"),
                        pre_interactions: vec![],
                        post_interactions: vec![],
                        sell_token_source: SellTokenSource::Erc20,
                        buy_token_destination: BuyTokenDestination::Erc20,
                    },
                    TradeKind::Legacy(LegacyTrade {
                        out_amount: U256::from(16380122291179526144u128),
                        gas_estimate: Some(225000),
                        interactions: vec![Interaction {
                            target: address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
                            data: const_hex::decode("aa77476c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000e357b42c3a9d8ccf0000000000000000000000000000000000000000000000000000000004d0e79e000000000000000000000000a69babef1ca67a37ffaf7a485dfff3382056e78c0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000066360af101ffffffffffffffffffffffffffffffffffffff0f3f47f166360a8d0000003f0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000001c66b3383f287dd9c85ad90e7c5a576ea4ba1bdf5a001d794a9afa379e6b2517b47e487a1aef32e75af432cbdbd301ada42754eaeac21ec4ca744afd92732f47540000000000000000000000000000000000000000000000000000000004d0c80f").unwrap(),
                            value: U256::ZERO,
                        }],
                        solver: address!("e3067c7c27c1038de4e8ad95a83b927d23dfbd99"),
                        tx_origin,
                    }),
                )
                .await
        }
    };

    let verified_quote = Estimate {
        out_amount: U256::from(16380122291179526144u128),
        gas: 225000,
        solver: address!("0xe3067c7c27c1038de4e8ad95a83b927d23dfbd99"),
        verified: true,
        execution: QuoteExecution {
            interactions: vec![InteractionData {
                target: address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
                value: U256::ZERO,
                call_data: const_hex::decode("aa77476c000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000e357b42c3a9d8ccf0000000000000000000000000000000000000000000000000000000004d0e79e000000000000000000000000a69babef1ca67a37ffaf7a485dfff3382056e78c0000000000000000000000009008d19f58aabd9ed0d60971565aa8510560ab41000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000066360af101ffffffffffffffffffffffffffffffffffffff0f3f47f166360a8d0000003f0000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000001c66b3383f287dd9c85ad90e7c5a576ea4ba1bdf5a001d794a9afa379e6b2517b47e487a1aef32e75af432cbdbd301ada42754eaeac21ec4ca744afd92732f47540000000000000000000000000000000000000000000000000000000004d0c80f").unwrap()
            }],
            pre_interactions: vec![],
            jit_orders: vec![],
        },
    };

    // `tx_origin: 0x0000` is currently used to bypass quote verification due to an
    // implementation detail of zeroex RFQ orders.
    // TODO: remove with #2693
    let verification = verify_trade(Some(Address::ZERO)).await;
    assert_eq!(&verification.unwrap(), &verified_quote);

    // Trades using any other `tx_origin` can not bypass the verification.
    let verification = verify_trade(None).await;
    assert_eq!(
        verification.unwrap(),
        Estimate {
            verified: false,
            ..verified_quote
        }
    );
}

/// Verified quotes work as for WETH trades without wrapping or approvals.
async fn verified_quote_eth_balance(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let weth = &onchain.contracts().weth;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // quote where the trader has no WETH balances or approval set, but
    // sufficient ETH for the trade
    assert!(
        weth.balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    assert!(
        weth.allowance(trader.address(), onchain.contracts().allowance)
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Test that asserts that we can verify quotes where the settlement contract is
/// the trader or receiver.
async fn verified_quote_for_settlement_contract(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(3u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;

    // Send 3 ETH to the settlement contract so we can get verified quotes for
    // selling WETH.
    onchain
        .send_wei(*onchain.contracts().gp_settlement.address(), 3u64.eth())
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    services.start_protocol(solver.clone()).await;

    let request = OrderQuoteRequest {
        sell_token: *onchain.contracts().weth.address(),
        buy_token: *token.address(),
        side: OrderQuoteSide::Sell {
            sell_amount: SellAmount::BeforeFee {
                value: (3u64.eth()).try_into().unwrap(),
            },
        },
        ..Default::default()
    };

    // quote where settlement contract is trader and implicit receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: *onchain.contracts().gp_settlement.address(),
            receiver: None,
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where settlement contract is trader and explicit receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: *onchain.contracts().gp_settlement.address(),
            receiver: Some(*onchain.contracts().gp_settlement.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where settlement contract is trader and not the receiver
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: *onchain.contracts().gp_settlement.address(),
            receiver: Some(trader.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where a random trader sends funds to the settlement contract
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            receiver: Some(*onchain.contracts().gp_settlement.address()),
            ..request.clone()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Test that asserts that we can verify quotes for traders with simulated
/// balances.
async fn verified_quote_with_simulated_balance(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(0u64.eth()).await;
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(1_000u64.eth(), 1_000u64.eth())
        .await;
    let weth = &onchain.contracts().weth;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;
    let (_config_file, config_arg) = Configuration::default().to_cli_args();
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec![
                    // The OpenZeppelin `ERC20Mintable` token uses a mapping in
                    // the first (0'th) storage slot for balances.
                    format!("--quote-token-balance-overrides={:?}@0", token.address()),
                    // We don't configure the WETH token and instead rely on
                    // auto-detection for balance overrides.
                    "--quote-autodetect-token-balance-overrides=true".to_string(),
                ],
                autopilot: vec![config_arg],
            },
            solver,
        )
        .await;

    // quote where the trader has no balances or approval set from TOKEN->WETH
    assert_eq!(
        (
            token.balanceOf(trader.address()).call().await.unwrap(),
            token
                .allowance(trader.address(), onchain.contracts().allowance)
                .call()
                .await
                .unwrap(),
        ),
        (U256::ZERO, U256::ZERO),
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *token.address(),
            buy_token: *weth.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // quote where the trader has no balances or approval set from WETH->TOKEN
    assert!(
        onchain
            .web3()
            .provider
            .get_balance(trader.address())
            .await
            .unwrap()
            .is_zero()
    );
    assert!(
        weth.balanceOf(trader.address())
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    assert!(
        weth.allowance(trader.address(), onchain.contracts().allowance)
            .call()
            .await
            .unwrap()
            .is_zero()
    );
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: trader.address(),
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // with balance overrides we can even verify quotes for the 0 address
    // which is used when no wallet is connected in the frontend
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: Address::ZERO,
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);

    // Previously quote verification did not set up the trade correctly
    // if the user provided pre-interactions. This works now.
    let response = services
        .submit_quote(&OrderQuoteRequest {
            from: Address::ZERO,
            sell_token: *weth.address(),
            buy_token: *token.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            app_data: model::order::OrderCreationAppData::Full {
                full: json!({
                    "metadata": {
                        "hooks": {
                            "pre": [
                                {
                                    "target": "0x0000000000000000000000000000000000000000",
                                    "callData": "0x",
                                    "gasLimit": "0"
                                }
                            ]
                        }
                    }
                })
                .to_string(),
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(response.verified);
}

/// Ensures that quotes can even be verified with tokens like `USDT`
/// which are not completely ERC20 compliant.
async fn usdt_quote_verification(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;

    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let usdt = address!("dac17f958d2ee523a2206206994597c13d831ec7");

    // Place Orders
    let services = Services::new(&onchain).await;
    let (_config_file, config_arg) = autopilot::config::Configuration::default().to_cli_args();
    services
        .start_protocol_with_args(
            ExtraServiceArgs {
                api: vec!["--quote-autodetect-token-balance-overrides=true".to_string()],
                autopilot: vec![config_arg],
            },
            solver,
        )
        .await;

    let quote = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: usdt,
            buy_token: usdc,
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1000u64.eth()).try_into().unwrap(),
                },
            },
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(quote.verified);
}

/// Tests that balance override detection works with debug_traceCall.
/// This test verifies the trace-based detection strategy that's similar to
/// Foundry's `deal`.
async fn trace_based_balance_detection(web3: Web3) {
    use shared::price_estimation::trade_verifier::balance_overrides::detector::Detector;

    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3.clone()).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    let [trader] = onchain.make_accounts(1u64.eth()).await;

    // Test with WETH (standard ERC20 with mapping at slot 3)
    let weth = *onchain.contracts().weth.address();

    // Deploy the NonStandardERC20Balances token - this has balances stored at an
    // offset within a struct mapping, making it undetectable by standard slot
    // calculation methods
    let struct_offset_token =
        contracts::test::NonStandardERC20Balances::Instance::deploy(web3.provider.clone())
            .await
            .unwrap();

    // Deploy the NonStandardERC20BalancesEntrance token - as if the previous
    // contract wasnt complicated enough, this contract will selectively
    // delegate the balance it returns between itself (allowing for testing of
    // calling another contract to get a balance--or calling another contract to
    // *not* get a balance)
    let local_storage_token =
        contracts::test::RemoteERC20Balances::Instance::deploy(web3.provider.clone(), weth, true)
            .await
            .unwrap();
    let delegated_storage_token =
        contracts::test::RemoteERC20Balances::Instance::deploy(web3.provider.clone(), weth, false)
            .await
            .unwrap();

    // Mint some tokens to the trader (so the contract has non-zero state)
    struct_offset_token
        .mint(trader.address(), 100u64.eth())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    local_storage_token
        .mint(trader.address(), 123u64.eth())
        .from(solver.address())
        .send_and_watch()
        .await
        .unwrap();

    let detector = Detector::new(web3.clone(), 60);

    let test_account = address!("0000000000000000000000000000000000000042");
    let test_balance = U256::from(123_456_789_u64);

    tracing::info!(?weth, "Testing WETH balance detection...");
    let weth_strategy = detector.detect(weth, test_account).await;
    tracing::info!("WETH strategy detected: {:?}", weth_strategy);
    assert!(
        matches!(weth_strategy, Ok(Strategy::SolidityMapping { .. })),
        "Should detect WETH balance slot via trace"
    );

    // Test with NonStandardERC20Balances - this is the key test case
    // The balance is at offset 2 within the UserData struct (epoch=0, approvals
    // mapping=1, balance=2)
    tracing::info!(address = ?struct_offset_token.address(), "Testing NonStandardERC20Balances detection...");
    let struct_offset_strategy = detector
        .detect(*struct_offset_token.address(), test_account)
        .await;
    assert!(
        matches!(struct_offset_strategy, Ok(Strategy::DirectSlot { .. })),
        "Should detect non-standard token balance slot via trace-based detection"
    );
    tracing::info!(
        "✓ NonStandardERC20Balances strategy detected: {:?}",
        struct_offset_strategy
    );

    tracing::info!(address = ?delegated_storage_token.address(), "Testing RemoteERC20Balances (using remote contract slot) detection...");
    let delegated_storage_strategy = detector
        .detect(*delegated_storage_token.address(), test_account)
        .await;
    assert!(
        matches!(
            delegated_storage_strategy,
            Ok(Strategy::SolidityMapping { .. })
        ),
        "Should detect non-standard token balance slot via trace-based detection"
    );
    tracing::info!(
        "✓ RemoteERC20Balances (remote) strategy detected: {:?}",
        delegated_storage_strategy
    );

    tracing::info!(address = ?local_storage_token.address(), "Testing RemoteERC20Balances (using local contract slot) detection...");
    let local_storage_strategy = detector
        .detect(*local_storage_token.address(), test_account)
        .await;
    assert!(
        matches!(local_storage_strategy, Ok(Strategy::DirectSlot { .. })),
        "Should detect non-standard token balance slot via trace-based detection"
    );
    tracing::info!(
        "✓ RemoteERC20Balances (self) strategy detected: {:?}",
        local_storage_strategy
    );

    // Verify that the detected strategies actually work by testing balance
    // overrides
    use contracts::ERC20;

    async fn test_balance_override(
        web3: &Web3,
        token: Address,
        strategy: Strategy,
        test_account: Address,
        test_balance: U256,
    ) {
        use {
            shared::price_estimation::trade_verifier::balance_overrides::BalanceOverrideRequest,
            std::collections::HashMap,
        };

        let balance_overrides = BalanceOverrides {
            hardcoded: HashMap::from([(token, strategy)]),
            detector: None,
        };

        let override_result = balance_overrides
            .state_override(BalanceOverrideRequest {
                token,
                holder: test_account,
                amount: test_balance,
            })
            .await;

        assert!(override_result.is_some(), "Should produce state override");
        let (override_token, state_override) = override_result.unwrap();

        // Call balanceOf with the state override to verify it works
        let token_contract = ERC20::Instance::new(token, web3.provider.clone());
        let balance = token_contract
            .balanceOf(test_account)
            .state(AddressMap::from_iter([(
                override_token,
                state_override.clone(),
            )]))
            .call()
            .await
            .unwrap();

        assert_eq!(
            balance, test_balance,
            "Balance override should work for token {:?}",
            token
        );

        tracing::info!(
            ?token,
            ?balance,
            ?override_token,
            ?state_override,
            "✓ Balance override verified for token",
        );
    }

    // Test each detected strategy
    test_balance_override(
        &web3,
        weth,
        weth_strategy.unwrap(),
        test_account,
        test_balance,
    )
    .await;
    test_balance_override(
        &web3,
        *struct_offset_token.address(),
        struct_offset_strategy.unwrap(),
        test_account,
        test_balance,
    )
    .await;
    test_balance_override(
        &web3,
        *delegated_storage_token.address(),
        delegated_storage_strategy.unwrap(),
        test_account,
        test_balance,
    )
    .await;
    test_balance_override(
        &web3,
        *local_storage_token.address(),
        local_storage_strategy.unwrap(),
        test_account,
        test_balance,
    )
    .await;
}
