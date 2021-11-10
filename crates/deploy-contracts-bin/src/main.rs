//! Deploy contracts to a local testnet for running services.

use anyhow::{Context, Result};
use shared::Web3;

#[tokio::main]
async fn main() -> Result<()> {
    const NODE_HOST: &str = "http://127.0.0.1:8545";
    let http = shared::transport::create_test_transport(NODE_HOST);
    let web3 = Web3::new(http);
    let contracts = contracts::deploy::Contracts::deploy(&web3)
        .await
        .context("deploy")?;
    let deployment = contracts.deployment();
    deployment.write().context("write deployment")?;
    Ok(())
}
