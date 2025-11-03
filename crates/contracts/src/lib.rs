#![allow(clippy::let_unit_value)]

pub use ethcontract;
pub mod alloy;
pub mod errors;
use {
    anyhow::{Result, anyhow, bail},
    ethcontract::{
        Contract,
        common::{DeploymentInformation, contract::Network},
    },
};

pub fn deployment(contract: &Contract, chain_id: u64) -> Result<&Network> {
    contract
        .networks
        .get(&chain_id.to_string())
        // Note that we are conflating network IDs with chain IDs. In general
        // they cannot be considered the same, but for the networks that we
        // support (xDAI, Görli and Mainnet) they are.
        .ok_or_else(|| anyhow!("missing {} deployment for {}", contract.name, chain_id))
}

pub fn deployment_block(contract: &Contract, chain_id: u64) -> Result<u64> {
    let deployment_info = deployment(contract, chain_id)?
        .deployment_information
        .ok_or_else(|| anyhow!("missing deployment information for {}", contract.name))?;

    match deployment_info {
        DeploymentInformation::BlockNumber(block) => Ok(block),
        DeploymentInformation::TransactionHash(tx) => {
            bail!("missing deployment block number for {}", tx)
        }
    }
}

#[macro_use]
mod macros;

#[cfg(feature = "bin")]
pub mod paths;
pub mod vault;
pub mod web3;
