use {
    autopilot::shutdown_controller::ShutdownController,
    e2e::setup::{OnchainComponents, Services, TIMEOUT, colocation, run_test, wait_for_condition},
    number::units::EthUnit,
    shared::ethrpc::Web3,
    std::{
        sync::{Arc, Mutex},
        time::Duration,
    },
};

#[tokio::test]
#[ignore]
async fn local_node_native_price_forwarding() {
    run_test(native_price_forwarding).await;
}

/// Test that native price forwarding from orderbook to autopilot works
/// correctly.
///
/// Architecture being tested:
///   User -> Orderbook (/api/v1/token/{token}/native_price)
///        -> Forwarder (configured in orderbook via `--native-price-estimators`)
///        -> Autopilot (/native_price/:token at port 12088)
///        -> Driver-based estimation
///        -> Returns price
///
/// The forwarding chain is configured in `crates/e2e/src/setup/services.rs`:
/// - Orderbook uses `Forwarder|http://localhost:12088` (see `api_autopilot_arguments`)
/// - Autopilot uses `Driver|test_quoter|http://localhost:11088/test_solver` (see
///   `autopilot_arguments`)
async fn native_price_forwarding(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;

    // Deploy token WITH UniV2 pool - this creates liquidity so price can be
    // estimated
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(500u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!("Starting services.");
    let services = Services::new(&onchain).await;

    // Start services manually (instead of start_protocol) to get shutdown control
    // over autopilot, allowing us to verify the forwarding dependency.
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

    let (shutdown_signal, shutdown_controller) = ShutdownController::new_manual_shutdown();
    let autopilot_handle = services
        .start_autopilot_with_shutdown_controller(
            None,
            vec![
                format!(
                    "--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
                    const_hex::encode(solver.address())
                ),
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--gas-estimators=http://localhost:11088/gasprice".to_string(),
            ],
            shutdown_controller,
        )
        .await;

    services
        .start_api(vec![
            "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
            "--gas-estimators=http://localhost:11088/gasprice".to_string(),
        ])
        .await;

    // Test 1: Token with liquidity returns a valid price via forwarding chain
    tracing::info!("Testing native price for token with liquidity.");
    let price_value = Arc::new(Mutex::new(-1.0));
    wait_for_condition(TIMEOUT, || async {
        match services.get_native_price(token.address()).await {
            Ok(p) => {
                *price_value.lock().unwrap() = p.price;
                true
            }
            _ => false,
        }
    })
    .await
    .expect("Expected successful price for token with liquidity");

    let price = *price_value.lock().unwrap();
    assert!(
        price > 0.0, // TODO: can we use a "close enough" approximation here, like we do for the weth case below? Just comparing for greater than zero seems lazy
        "Price should have been set to a positive value"
    );
    tracing::info!(price, "Got native price for token");

    // Test 2: WETH (native token) returns price of ~1.0
    tracing::info!("Testing native price for WETH.");
    let weth_price_value = Arc::new(Mutex::new(-1.0));
    wait_for_condition(TIMEOUT, || async {
        match services
            .get_native_price(onchain.contracts().weth.address())
            .await
        {
            Ok(p) => {
                *weth_price_value.lock().unwrap() = p.price;
                true
            }
            _ => false,
        }
    })
    .await
    .expect("Expected successful price for WETH");

    let weth_price = *weth_price_value.lock().unwrap();
    assert!(weth_price >= 0.0, "WETH price should have been set");

    // WETH price should be ~1.0, since it is the native token
    assert!(
        (weth_price - 1.0).abs() < 1e-6,
        "WETH price should be ~1.0, got {}",
        weth_price
    );
    tracing::info!(weth_price, "Got native price for WETH");

    // Test 3: Stop autopilot and verify price estimation fails (proves forwarding dependency)
    tracing::info!("Stopping autopilot to verify forwarding dependency.");
    shutdown_signal.shutdown();
    // Autopilot checks shutdown signal during its run loop, which is triggered by new blocks
    onchain.mint_block().await;
    tokio::time::timeout(Duration::from_secs(15), autopilot_handle)
        .await
        .expect("autopilot should shut down within timeout")
        .expect("autopilot task should complete without panic");

    // Wait for native price cache to expire (configured as 2s in services.rs)
    tracing::info!("Waiting for native price cache to expire.");
    tokio::time::sleep(Duration::from_secs(3)).await;

    tracing::info!("Verifying native price fails without autopilot.");
    let result = services.get_native_price(token.address()).await;
    assert!(
        result.is_err(),
        "Expected price request to fail after autopilot shutdown, proving the forwarding \
         dependency. Got: {:?}",
        result
    );
    tracing::info!("Confirmed: orderbook forwards native price requests to autopilot");
}
