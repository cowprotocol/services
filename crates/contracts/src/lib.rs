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

macro_rules! include_contracts {
    ($($name:ident;)*) => {$(
        include!(concat!(env!("OUT_DIR"), "/", stringify!($name), ".rs"));
    )*};
}

include_contracts! {
    ERC20;
    GPv2Settlement;
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::alloy::networks::{ARBITRUM_ONE, GNOSIS, MAINNET, SEPOLIA},
    };

    #[test]
    fn deployment_addresses() {
        for network in &[MAINNET, GNOSIS, SEPOLIA, ARBITRUM_ONE] {
            assert!(
                alloy::BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory::deployment_address(network).is_some()
            )
        }
        for network in &[MAINNET, ARBITRUM_ONE] {
            assert!(
                alloy::BalancerV2WeightedPool2TokensFactory::deployment_address(network).is_some()
            );
            assert!(
                alloy::BalancerV2LiquidityBootstrappingPoolFactory::deployment_address(network)
                    .is_some()
            );
        }

        assert!(alloy::BalancerV2WeightedPoolFactory::deployment_address(&MAINNET).is_some());

        for network in &[MAINNET, GNOSIS, ARBITRUM_ONE] {
            assert!(alloy::BalancerV2StablePoolFactoryV2::deployment_address(network).is_some());
        }
    }

    #[test]
    fn deployment_information() {
        assert!(alloy::BalancerV2WeightedPoolFactory::deployment_address(&MAINNET).is_some());
        for network in &[MAINNET, ARBITRUM_ONE] {
            assert!(
                alloy::BalancerV2WeightedPool2TokensFactory::deployment_address(network).is_some()
            );
        }
        for network in &[MAINNET, GNOSIS, ARBITRUM_ONE] {
            assert!(alloy::BalancerV2StablePoolFactoryV2::deployment_address(network).is_some());
        }
    }
}
