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
    async fn setup(onchain: &'a OnchainComponents, solver: TestAccount) -> Self {
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

        let services = Services::new(onchain).await;

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
                "--price-estimation-drivers=test_quoter|http://localhost:11088/test_solver"
                    .to_string(),
                "--gas-estimators=http://localhost:11088/gasprice".to_string(),
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
            match services.get_native_price(token).await {
                Ok(p) => {
                    *price.lock().await = p.price;
                    true
                }
                _ => false,
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
///   User -> Orderbook (/api/v1/token/{token}/native_price)
///        -> Forwarder (configured in orderbook via
/// `--native-price-estimators`)        -> Autopilot (/native_price/:token at
/// port 12088)        -> Driver-based estimation
///        -> Returns price
///
/// The forwarding chain is configured in `crates/e2e/src/setup/services.rs`:
/// - Orderbook uses `Forwarder|http://localhost:12088` (see
///   `api_autopilot_arguments`)
/// - Autopilot uses `Driver|test_quoter|http://localhost:11088/test_solver`
///   (see `autopilot_arguments`)
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

    // Test 3: Stop autopilot and verify price estimation fails (proves forwarding
    // dependency)
    tracing::info!("Stopping autopilot to verify forwarding dependency.");
    let services = env.shutdown_autopilot(&onchain).await;

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
