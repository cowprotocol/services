use {
    e2e::setup::*,
    number::units::EthUnit,
    shared::web3::Web3,
};

#[tokio::test]
#[ignore]
async fn pod_test_shadow_mode() {
    run_pod_test(pod_shadow_mode_test).await;
}

async fn pod_shadow_mode_test(web3: Web3) {
    tracing::info!("Setting up chain state for pod shadow mode test.");
    let mut onchain = OnchainComponents::deploy(web3).await;

    let [solver] = onchain.make_solvers(10u64.eth()).await;
    tracing::info!(?solver, "Created solver account");

    tracing::info!("Starting services with pod-enabled driver.");
    let services = Services::new(&onchain).await;
    services.start_protocol_with_pod(solver.clone()).await;

    tracing::info!("Pod-enabled driver started successfully.");

    // Give services a moment to fully initialize
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    tracing::info!("Pod shadow mode test completed successfully.");
}
