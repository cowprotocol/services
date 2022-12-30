use crate::{infra, infra::config::cli, tests::setup};

/// Test that the example solvers config file is valid by checking that the
/// driver does not crash when started with this file.
#[tokio::test]
async fn test() {
    let example_config_file = std::env::current_dir()
        .unwrap()
        .join("example.solvers.toml");
    setup::driver::setup(setup::driver::Config {
        now: infra::time::Now::Real,
        contracts: cli::ContractAddresses {
            gp_v2_settlement: Some("0x0000000000000000000000000000000000000000".to_owned()),
            weth: Some("0x0000000000000000000000000000000000000000".to_owned()),
        },
        solvers: setup::driver::SolversConfig::LoadConfigFile(example_config_file),
    })
    .await;
}
