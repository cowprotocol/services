use crate::{infra, tests::setup};

/// Test that the example configuration file is valid by checking that the
/// driver does not crash when started with this file.
#[ignore]
#[tokio::test]
async fn test() {
    let geth = setup::blockchain::geth().await;
    let example_config_file = std::env::current_dir().unwrap().join("example.toml");
    setup::driver::setup(setup::driver::Config {
        now: infra::time::Now::Real,
        file: setup::driver::ConfigFile::Load(example_config_file),
        geth: &geth,
    })
    .await;
}
