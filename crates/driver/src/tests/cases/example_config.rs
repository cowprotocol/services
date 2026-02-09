use {crate::tests, alloy::primitives::address};

/// Test that the example configuration file is valid by checking that the
/// driver does not crash when started with this file.
#[tokio::test]
#[ignore]
async fn test() {
    let example_config_file = std::env::current_dir().unwrap().join("example.toml");
    tests::setup()
        .config(example_config_file)
        .settlement_address(address!("9008D19f58AAbD9eD0D60971565AA8510560ab41"))
        .balances_address(address!("3e8C6De9510e7ECad902D005DE3Ab52f35cF4f1b"))
        .signatures_address(address!("8262d639c38470F38d2eff15926F7071c28057Af"))
        .done()
        .await;
}
