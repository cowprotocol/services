use {
    ::alloy::{
        primitives::{Address, address},
        providers::{
            Provider,
            ext::{AnvilApi, DebugApi, ImpersonateConfig},
        },
        rpc::types::trace::geth::{CallConfig, GethDebugTracingOptions},
    },
    app_data::{AppDataHash, hash_full_app_data},
    contracts::alloy::ERC20,
    e2e::setup::*,
    ethrpc::alloy::CallBuilderExt,
    model::{
        order::{OrderCreation, OrderCreationAppData, OrderKind},
        quote::{OrderQuoteRequest, OrderQuoteSide, SellAmount},
        signature::EcdsaSigningScheme,
    },
    number::units::EthUnit,
    serde_json::json,
    shared::web3::Web3,
};

/// The block number from which we will fetch state for the forked test.
const FORK_BLOCK_MAINNET: u64 = 23688436;

/// EmptyWrapper contract address deployed on mainnet.
const EMPTY_WRAPPER_MAINNET: Address = address!("751871E9cA28B441Bb6d3b7C4255cf2B5873d56a");

#[tokio::test]
#[ignore]
async fn forked_node_mainnet_wrapper() {
    run_forked_test_with_block_number(
        forked_mainnet_wrapper_test,
        std::env::var("FORK_URL_MAINNET")
            .expect("FORK_URL_MAINNET must be set to run forked tests"),
        FORK_BLOCK_MAINNET,
    )
    .await;
}

/// Test that orders can be placed with wrapper contracts specified in the app
/// data.
async fn forked_mainnet_wrapper_test(web3: Web3) {
    let mut onchain = OnchainComponents::deployed(web3.clone()).await;

    let [solver] = onchain.make_solvers_forked(1u64.eth()).await;
    let [trader] = onchain.make_accounts(2u64.eth()).await;

    let token_weth = onchain.contracts().weth.clone();
    let token_usdc = ERC20::Instance::new(
        address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        web3.provider.clone(),
    );

    // Authorize the empty wrapper as a solver
    web3.provider
        .anvil_send_impersonated_transaction_with_config(
            onchain
                .contracts()
                .gp_authenticator
                .addSolver(EMPTY_WRAPPER_MAINNET)
                .from(
                    onchain
                        .contracts()
                        .gp_authenticator
                        .manager()
                        .call()
                        .await
                        .unwrap(),
                )
                .into_transaction_request(),
            ImpersonateConfig {
                fund_amount: Some(1u64.eth()),
                stop_impersonate: true,
            },
        )
        .await
        .unwrap()
        .get_receipt()
        .await
        .unwrap();

    // Trader deposits ETH to get WETH
    token_weth
        .deposit()
        .value(1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Approve GPv2 for trading
    token_weth
        .approve(onchain.contracts().allowance, 1u64.eth())
        .from(trader.address())
        .send_and_watch()
        .await
        .unwrap();

    // Start services
    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    onchain.mint_block().await;

    // Create app data with the deployed EmptyWrapper contract
    let app_data = json!({
        "version": "0.9.0",
        "metadata": {
            "wrappers": [
                {
                    "address": format!("{:?}", EMPTY_WRAPPER_MAINNET),
                    "data": "0xbeef",
                    "isOmittable": false,
                },
                {
                    "address": format!("{:?}", EMPTY_WRAPPER_MAINNET),
                    "data": "0xfeed",
                    "isOmittable": false,
                },
            ]
        }
    })
    .to_string();

    let app_data_hash = AppDataHash(hash_full_app_data(app_data.as_bytes()));

    // Warm up co-located driver by quoting the order
    let _ = services
        .submit_quote(&OrderQuoteRequest {
            sell_token: *token_weth.address(),
            buy_token: *token_usdc.address(),
            side: OrderQuoteSide::Sell {
                sell_amount: SellAmount::BeforeFee {
                    value: (1u64.eth()).try_into().unwrap(),
                },
            },
            app_data: OrderCreationAppData::Both {
                full: app_data.clone(),
                expected: app_data_hash,
            },
            ..Default::default()
        })
        .await;

    tracing::info!("Creating order with wrapper in app data");
    let order = OrderCreation {
        app_data: OrderCreationAppData::Both {
            full: app_data.clone(),
            expected: app_data_hash,
        },
        sell_token: *token_weth.address(),
        sell_amount: 1u64.eth(),
        buy_token: *token_usdc.address(),
        buy_amount: ::alloy::primitives::U256::ONE,
        valid_to: model::time::now_in_epoch_seconds() + 300,
        kind: OrderKind::Sell,
        ..Default::default()
    }
    .sign(
        EcdsaSigningScheme::Eip712,
        &onchain.contracts().domain_separator,
        &trader.signer,
    );

    let sell_token_balance_before = token_weth.balanceOf(trader.address()).call().await.unwrap();
    let buy_token_balance_before = token_usdc.balanceOf(trader.address()).call().await.unwrap();

    // Create the order
    let order_uid = services.create_order(&order).await.unwrap();
    tracing::info!("Order created with UID: {:?}", order_uid);

    // Verify the order was created with correct app data
    let created_order = services.get_order(&order_uid).await.unwrap();
    assert_eq!(created_order.data.app_data, app_data_hash);
    assert_eq!(
        created_order.metadata.full_app_data.as_deref(),
        Some(app_data.as_str())
    );

    // Verify app data can be retrieved
    let retrieved_app_data = services.get_app_data(app_data_hash).await.unwrap();
    assert_eq!(retrieved_app_data, app_data);

    // Drive solution
    tracing::info!("Waiting for trade.");

    wait_for_condition(TIMEOUT, || async {
        onchain.mint_block().await;
        let sell_token_balance_after = token_weth.balanceOf(trader.address()).call().await.unwrap();
        let buy_token_balance_after = token_usdc.balanceOf(trader.address()).call().await.unwrap();

        (sell_token_balance_before > sell_token_balance_after)
            && (buy_token_balance_after > buy_token_balance_before)
    })
    .await
    .unwrap();

    tracing::info!("Transaction completion observed.");

    wait_for_condition(TIMEOUT, || async {
        // It takes a bit of extra time for the API to receive the actual txn
        let trades_result = services.get_trades(&order_uid).await.unwrap();
        !trades_result.is_empty()
    })
    .await
    .unwrap();

    // slight repeating of code here because accessing the data from within the
    // `wait_for_condition` turns out to be difficult
    let trades_result = services.get_trades(&order_uid).await.unwrap();
    let solve_tx_hash = trades_result[0].tx_hash.unwrap();
    tracing::info!("Settlement transaction hash: {:?}", solve_tx_hash);

    let solve_tx = web3
        .provider
        .get_transaction_by_hash(solve_tx_hash)
        .await
        .unwrap()
        .unwrap()
        .into_request();

    // the call itself should have gone to the wrapper
    assert_eq!(
        solve_tx.to.unwrap().into_to().unwrap(),
        EMPTY_WRAPPER_MAINNET
    );

    // Trace the transaction to verify both wrapper calls happened
    tracing::info!("Tracing transaction to verify wrapper calls");

    // Create GethDebugTracingOptions with callTracer explicitly set
    let tracing_options = GethDebugTracingOptions {
        tracer: Some(
            ::alloy::rpc::types::trace::geth::GethDebugTracerType::BuiltInTracer(
                ::alloy::rpc::types::trace::geth::GethDebugBuiltInTracerType::CallTracer,
            ),
        ),
        tracer_config: serde_json::to_value(CallConfig::default())
            .ok()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap(),
        ..Default::default()
    };

    let trace = web3
        .provider
        .debug_trace_transaction(solve_tx_hash, tracing_options)
        .await
        .unwrap();

    // Extract call frame from the trace
    let call_frame = match trace {
        ::alloy::rpc::types::trace::geth::GethTrace::CallTracer(frame) => frame,
        other => panic!("Expected CallTracer trace but got: {:?}", other),
    };

    // Verify that we have calls to the wrapper with the expected data
    let mut wrapper_calls = Vec::new();
    fn collect_wrapper_calls(
        frame: &::alloy::rpc::types::trace::geth::CallFrame,
        wrapper_addr: Address,
        calls: &mut Vec<::alloy::primitives::Bytes>,
    ) {
        if frame.to == Some(wrapper_addr) {
            calls.push(frame.input.clone());
        }
        for call in &frame.calls {
            collect_wrapper_calls(call, wrapper_addr, calls);
        }
    }
    collect_wrapper_calls(&call_frame, EMPTY_WRAPPER_MAINNET, &mut wrapper_calls);

    tracing::info!(
        "Found {} wrapper calls in transaction trace",
        wrapper_calls.len()
    );
    assert_eq!(
        wrapper_calls.len(),
        2,
        "Expected 2 wrapper calls but found {}",
        wrapper_calls.len()
    );

    // Verify the wrapper calls contain the expected data (0xbeef and 0xfeed)
    let call_data_strings: Vec<String> = wrapper_calls
        .iter()
        .map(|data| format!("{:?}", data))
        .collect();

    assert!(
        call_data_strings[0].contains("0002beef"),
        "Initial call data does not contain first wrapper data"
    );
    assert!(
        call_data_strings[1].contains("0002feed"),
        "Initial call data does not contain second wrapper data"
    );
    tracing::info!("Wrapper call data: {:?}", call_data_strings);

    // Check that the auction ID propogated through the wrappers ok
    // Sometimes the API isnt ready to respond to the request immediately so we wait
    // a bit for success
    wait_for_condition(TIMEOUT, || async {
        let auction_info = services.get_solver_competition(solve_tx_hash).await;

        if let Ok(a) = auction_info {
            tracing::info!("Pulled auction id {:?}", a.auction_id);
            true
        } else {
            false
        }
    })
    .await
    .unwrap();

    tracing::info!(
        "Order with wrapper successfully traded on forked mainnet with verified wrapper calls"
    );
}
