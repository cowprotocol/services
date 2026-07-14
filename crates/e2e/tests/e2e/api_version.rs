use {e2e::setup::*, number::units::EthUnit, shared::web3::Web3};

#[tokio::test]
#[ignore]
async fn local_node_api_version() {
    run_test(api_version).await;
}

/// Test that the API version endpoint returns a version string
async fn api_version(web3: Web3) {
    unsafe {
        std::env::set_var("GIT_SHA", "2491c6a");
        std::env::set_var("GIT_BRANCH", "branch");
    }

    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Get the API version
    let version = services.get_api_version().await.unwrap();

    assert_eq!(version, "branch@2491c6a");
}
