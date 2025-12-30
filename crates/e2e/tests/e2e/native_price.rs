use {
    alloy::primitives::Address,
    autopilot::shutdown_controller::{ShutdownController, ShutdownSignal},
    e2e::setup::{
        OnchainComponents,
        Services,
        TIMEOUT,
        TestAccount,
        colocation,
        run_test,
        wait_for_condition,
    },
    number::units::EthUnit,
    shared::ethrpc::Web3,
    std::{sync::Arc, time::Duration},
    tokio::{sync::Mutex, task::JoinHandle},
};

/// Test environment with shutdown control over autopilot.
struct TestEnv<'a> {
    services: Services<'a>,
    shutdown_signal: Option<ShutdownSignal>,
    autopilot_handle: JoinHandle<()>,
}

impl<'a> TestEnv<'a> {
    /// Sets up driver, autopilot (with shutdown control), and orderbook API.
    ///
    /// NOTE: This setup explicitly specifies configuration values required for
    /// its assertions, even when they match the current defaults in the
    /// Services infrastructure. This intentional redundancy decouples the
    /// test's correctness from the default configuration. If defaults
    /// change in the future, this test will (hopefully) continue to validate
    /// the same behavior without silent breakage.
    async fn setup(onchain: &'a OnchainComponents, solver: TestAccount) -> Self {
        // Start the driver service
        let test_solver = colocation::start_baseline_solver(
            "test_solver".into(),
            solver.clone(),
            *onchain.contracts().weth.address(),
            vec![],
            1,
            true,
        )
        .await;
        colocation::start_driver(
            onchain.contracts(),
            vec![test_solver],
            colocation::LiquidityProvider::UniswapV2,
            false,
        );

        let services = Services::new(onchain).await;

        let (shutdown_signal, shutdown_controller) = ShutdownController::new_manual_shutdown();

        // Repeating the standard configuration matching `start_protocol()` in
        // services.rs for a minimal working setup.
        //
        // TODO: Maybe we should make this configurable from the outside?
        let autopilot_handle = services
            .start_autopilot_with_shutdown_controller(
                None,
                vec![
                    // Register the test solver
                    format!(
                        "--drivers=test_solver|http://localhost:11088/test_solver|{}|requested-timeout-on-problems",
                        const_hex::encode(solver.address())
                    ),
                    // Configure driver-based price estimation (points to the same solver endpoint for quotes)
                    "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver".to_string(),
                    // Configure where to get gas price estimates
                    "--gas-estimators=http://localhost:11088/gasprice".to_string(),
                ],
                shutdown_controller,
            )
            .await;

        // Start the orderbook API service
        services
            .start_api(vec![
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--gas-estimators=http://localhost:11088/gasprice".to_string(),
                "--native-price-cache-max-age=2s".to_string(),
                "--native-price-estimators=Forwarder|http://localhost:12088".to_string(),
            ])
            .await;

        Self {
            services,
            shutdown_signal: Some(shutdown_signal),
            autopilot_handle,
        }
    }

    /// Gracefully shuts down autopilot and waits for it to complete.
    /// Returns the services so they can continue to be used after shutdown.
    async fn shutdown_autopilot(mut self, onchain: &OnchainComponents) -> Services<'a> {
        let signal = self
            .shutdown_signal
            .take()
            .expect("shutdown already called");
        signal.shutdown();
        // Autopilot checks shutdown signal during its run loop, triggered by new blocks
        onchain.mint_block().await;
        tokio::time::timeout(Duration::from_secs(15), self.autopilot_handle)
            .await
            .expect("autopilot should shut down within timeout")
            .expect("autopilot task should complete without panic");
        self.services
    }
}

/// Waits for a native price to become available and returns it.
async fn wait_for_price(services: &Services<'_>, token: &Address) -> f64 {
    let price = Arc::new(Mutex::new(-1.0));
    wait_for_condition(TIMEOUT, || {
        let price = price.clone();
        async move {
            if let Ok(p) = services.get_native_price(token).await {
                *price.lock().await = p.price;
                true
            } else {
                false
            }
        }
    })
    .await
    .expect("Expected successful price");
    *price.lock().await
}

#[tokio::test]
#[ignore]
async fn local_node_native_price_forwarding() {
    run_test(native_price_forwarding).await;
}

/// Test that native price forwarding from orderbook to autopilot works
/// correctly.
///
/// Architecture being tested:
///
/// User Request
/// -> Orderbook (port 8080, /api/v1/token/{token}/native_price)
/// -> Forwarder (configured in orderbook via `--native-price-estimators`)
/// -> Autopilot (port 12088, /native_price/:token)
/// -> Driver (port 11088, /test_solver)
/// -> Returns price
///
/// Two-hop forwarding chain:
/// - Hop 1: Orderbook → Autopilot (by `Forwarder|http://localhost:12088`)
/// - Hop 2: Autopilot → Driver (by ``Driver|test_quoter|http://localhost:11088/test_solver``)
async fn native_price_forwarding(web3: Web3) {
    tracing::info!("Setting up chain state.");
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(10u64.eth()).await;

    // Deploy token WITH UniV2 pool.
    // This creates liquidity so price can ben estimated.
    let [token] = onchain
        .deploy_tokens_with_weth_uni_v2_pools(500u64.eth(), 1_000u64.eth())
        .await;

    tracing::info!("Starting services.");
    let env = TestEnv::setup(&onchain, solver).await;

    // Test 1: Token with liquidity returns a valid price via forwarding chain
    tracing::info!("Testing native price for token with liquidity.");
    let price = wait_for_price(&env.services, token.address()).await;
    assert!(price > 0.0, "Price should be positive, got {}", price);
    tracing::info!(price, "Got native price for token");

    // Test 2: WETH (native token) returns price of ~1.0
    tracing::info!("Testing native price for WETH.");
    let weth_price = wait_for_price(&env.services, onchain.contracts().weth.address()).await;
    assert!(
        (weth_price - 1.0).abs() < 1e-6,
        "WETH price should be ~1.0, got {}",
        weth_price
    );
    tracing::info!(weth_price, "Got native price for WETH");

    // Test 3: Stop autopilot and verify price estimation fails.
    // By stopping autopilot and showing that native price requests fail, we prove
    // the following:
    // - Orderbook depends on autopilot for native prices
    // - The Forwarder configuration is actually being used
    // - There's no fallback mechanism that would mask configuration issues:
    //   - If someone accidentally added a fallback estimator, this test would catch
    //     it (because the request would succeed)

    tracing::info!("Stopping autopilot to verify forwarding dependency.");
    let services = env.shutdown_autopilot(&onchain).await;

    // Wait for native price cache to expire (explicitly configured as 2s in this
    // test setup)
    tracing::info!("Waiting for native price cache to expire.");
    tokio::time::sleep(Duration::from_secs(3)).await;

    tracing::info!("Verifying native price fails without autopilot.");
    let result = services.get_native_price(token.address()).await;
    let (status, body) =
        result.expect_err("Expected price request to fail after autopilot shutdown");

    // EstimatorInternal errors (connection refused) are mapped to 404 "NoLiquidity"
    // in the orderbook API (see crates/orderbook/src/api.rs:381-383)
    assert!(
        status == reqwest::StatusCode::NOT_FOUND,
        "Expected 404 status after autopilot shutdown, got {}",
        status
    );
    assert!(
        body.contains("NoLiquidity"),
        "Expected NoLiquidity error after autopilot shutdown, got: {}",
        body
    );

    tracing::info!("Confirmed: orderbook forwards native price requests to autopilot");
}
