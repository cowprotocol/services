pub mod networks {
    pub const MAINNET: u64 = 1;
    pub const GNOSIS: u64 = 100;
    pub const SEPOLIA: u64 = 11155111;
    pub const ARBITRUM_ONE: u64 = 42161;
    pub const BASE: u64 = 8453;
    pub const POLYGON: u64 = 137;
    pub const AVALANCHE: u64 = 43114;
    pub const BNB: u64 = 56;
    pub const OPTIMISM: u64 = 10;
    pub const LENS: u64 = 232;
}

crate::bindings!(
    ChainalysisOracle,
    maplit::hashmap! {
        MAINNET => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        ARBITRUM_ONE => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        BASE => address!("0x3A91A31cB3dC49b4db9Ce721F50a9D076c8D739B"),
        AVALANCHE => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        BNB => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        OPTIMISM => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        POLYGON => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
    }
);

pub use alloy::providers::DynProvider as Provider;

/// Extension trait to attach some useful functions to the contract instance.
pub trait InstanceExt: Sized {
    /// Crates a contract instance at the expected address for the current
    /// network.
    fn deployed(
        provider: &Provider,
    ) -> impl std::future::Future<Output = anyhow::Result<Self>> + Send;
}

#[macro_export]
macro_rules! bindings {
    ($contract:ident, $($deployment_info:expr)?) => {
        paste::paste! {
            // Generate the main bindings in a private module. That allows
            // us to re-export all items in our own module while also adding
            // some items ourselves.
            #[allow(non_snake_case)]
            mod [<$contract Private>] {
                alloy::sol!(
                    #[allow(missing_docs)]
                    #[sol(rpc)]
                    $contract,
                    concat!("./artifacts/", stringify!($contract), ".json"),
                );
            }

            #[allow(non_snake_case)]
            pub mod $contract {
                use alloy::providers::DynProvider;

                pub use super::[<$contract Private>]::*;
                pub type Instance = $contract::[<$contract Instance>]<DynProvider>;

                $(
                use {
                    std::{sync::LazyLock, collections::HashMap},
                    alloy::{
                        providers::Provider,
                        primitives::{address, Address},
                    },
                    anyhow::{Context, Result},
                    $crate::alloy::networks::*,
                };

                pub static DEPLOYMENT_INFO: LazyLock<HashMap<u64, Address>> = LazyLock::new(|| {
                    $deployment_info
                });

                impl $crate::alloy::InstanceExt for Instance {
                    fn deployed(provider: &DynProvider) -> impl Future<Output = Result<Self>> + Send {
                        async move {
                            let chain_id = provider
                                .get_chain_id()
                                .await
                                .context("could not fetch current chain id")?;
                            let address = DEPLOYMENT_INFO
                                .get(&chain_id)
                                .with_context(|| format!("no deployment info for chain {chain_id:?}"))?;

                            Ok(Instance::new(
                                address.clone(),
                                provider.clone(),
                            ))
                        }
                    }
                }
                )*
            }
        }
    };
}
