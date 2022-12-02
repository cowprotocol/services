use anyhow::{anyhow, bail, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    common::{contract::Network, DeploymentInformation},
    Contract,
};
use web3::types::U64;

use crate::{
    current_block::{block_number_to_block_number_hash, BlockNumberHash},
    ethrpc::Web3,
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

pub async fn deployment_block(contract: &Contract, chain_id: u64) -> Result<u64> {
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

pub async fn settlement_deployment_block_number_hash(
    web3: &Web3,
    chain_id: u64,
) -> Result<BlockNumberHash> {
    let block_number = deployment_block(GPv2Settlement::raw_contract(), chain_id).await?;
    block_number_to_block_number_hash(web3, U64::from(block_number).into())
        .await
        .ok_or_else(|| anyhow!("Deployment block not found"))
}
