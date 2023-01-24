use crate::{infra, infra::config::cli, tests::setup};

/// Test that the example configuration file is valid by checking that the
/// driver does not crash when started with this file.
#[ignore]
#[tokio::test]
async fn test() {
    let geth = setup::blockchain::geth().await;
    let example_config_file = std::env::current_dir().unwrap().join("example.toml");
    setup::driver::setup(setup::driver::Config {
        now: infra::time::Now::Real,
        contracts: cli::ContractAddresses {
            gp_v2_settlement: Some(Default::default()),
            weth: Some(Default::default()),
        },
        file: setup::driver::ConfigFile::Load(example_config_file),
        geth: &geth,
    })
    .await;
}
