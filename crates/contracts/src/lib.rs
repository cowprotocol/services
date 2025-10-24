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
    CowAmm;
    CowAmmConstantProductFactory;
    CowAmmLegacyHelper;
    CowAmmUniswapV2PriceOracle;
    CowProtocolToken;
    ERC20;
    GPv2AllowListAuthentication;
    GPv2Settlement;
    WETH9;
}

#[cfg(test)]
mod tests {
    use crate::alloy::networks::{ARBITRUM_ONE, GNOSIS, MAINNET, SEPOLIA};
    use {
        super::*,
        ethcontract::{
            common::DeploymentInformation,
            futures::future::{self, FutureExt as _, Ready},
            json::json,
            jsonrpc::{Call, Id, MethodCall, Params, Value},
            web3::{BatchTransport, RequestId, Transport, Web3, error::Result as Web3Result},
        },
    };

    #[derive(Debug, Clone)]
    struct ChainIdTransport(u64);

    impl Transport for ChainIdTransport {
        type Out = Ready<Web3Result<Value>>;

        fn prepare(&self, method: &str, params: Vec<Value>) -> (RequestId, Call) {
            assert_eq!(method, "eth_chainId");
            assert_eq!(params.len(), 0);
            (
                0,
                MethodCall {
                    jsonrpc: None,
                    method: method.to_string(),
                    params: Params::Array(params),
                    id: Id::Num(0),
                }
                .into(),
            )
        }

        fn send(&self, _id: RequestId, _request: Call) -> Self::Out {
            future::ready(Ok(json!(format!("{:x}", self.0))))
        }
    }

    impl BatchTransport for ChainIdTransport {
        type Batch = Ready<Web3Result<Vec<Web3Result<Value>>>>;

        fn send_batch<T>(&self, requests: T) -> Self::Batch
        where
            T: IntoIterator<Item = (RequestId, Call)>,
        {
            future::ready(Ok(requests
                .into_iter()
                .map(|_| Ok(json!(format!("{:x}", self.0))))
                .collect()))
        }
    }

    #[test]
    fn deployment_addresses() {
        macro_rules! assert_has_deployment_address {
            ($contract:ident for $network:expr_2021) => {{
                let web3 = Web3::new(ChainIdTransport($network));
                let deployed = $contract::deployed(&web3).now_or_never().unwrap();
                assert!(deployed.is_ok());
            }};
        }

        for network in &[MAINNET, GNOSIS, SEPOLIA, ARBITRUM_ONE] {
            assert_has_deployment_address!(GPv2Settlement for *network);
            assert_has_deployment_address!(WETH9 for *network);
            assert!(
                alloy::BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory::deployment_address(network).is_some()
            )
        }
        for network in &[MAINNET, GNOSIS, SEPOLIA] {
            assert_has_deployment_address!(CowProtocolToken for *network);
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
        macro_rules! assert_has_deployment_information {
            ($contract:ident for $network:expr_2021) => {{
                let web3 = Web3::new(ChainIdTransport($network));
                let instance = $contract::deployed(&web3).now_or_never().unwrap().unwrap();
                assert!(matches!(
                    instance.deployment_information(),
                    Some(DeploymentInformation::BlockNumber(_)),
                ));
            }};
        }

        for network in &[MAINNET, GNOSIS, SEPOLIA, ARBITRUM_ONE] {
            assert_has_deployment_information!(GPv2Settlement for *network);
        }
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
