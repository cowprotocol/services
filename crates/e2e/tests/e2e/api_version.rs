use {e2e::setup::*, number::units::EthUnit, shared::web3::Web3};

#[tokio::test]
#[ignore]
async fn local_node_api_version() {
    run_test(api_version).await;
}

/// Test that the API version endpoint returns a version string
async fn api_version(web3: Web3) {
    let mut onchain = OnchainComponents::deploy(web3).await;
    let [solver] = onchain.make_solvers(1u64.eth()).await;

    let services = Services::new(&onchain).await;
    services.start_protocol(solver).await;

    // Get the API version
    let version = services.get_api_version().await.unwrap();

    // Version should match git describe format
    // Format examples: v1.2.3, v1.2.3-4-gabcd1234, or abcd1234
    let is_valid_version = version.starts_with('v')
        || version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');

    assert!(
        is_valid_version,
        "Version should be valid git describe format: {version}"
    );
}
