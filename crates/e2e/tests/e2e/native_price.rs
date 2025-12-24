use {
    e2e::setup::{OnchainComponents, Services, TIMEOUT, run_test, wait_for_condition},
    number::units::EthUnit,
    shared::ethrpc::Web3,
    std::sync::{Arc, Mutex},
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
    services.start_protocol(solver).await;

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
}
