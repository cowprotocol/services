use crate::{infra, infra::config::cli, tests::setup};

/// Test that the example solvers config file is valid by checking that the
/// driver does not crash when started with this file.
#[ignore]
#[tokio::test]
async fn test() {
    let example_config_file = std::env::current_dir()
        .unwrap()
        .join("example.solvers.toml");
    setup::driver::setup(setup::driver::Config {
        now: infra::time::Now::Real,
        contracts: cli::ContractAddresses {
            gp_v2_settlement: Some(Default::default()),
            weth: Some(Default::default()),
        },
        solvers: setup::driver::SolversConfig::LoadConfigFile(example_config_file),
    })
    .await;
}
