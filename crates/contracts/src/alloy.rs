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
    crate::deployments! {
        MAINNET => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        ARBITRUM_ONE => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        BASE => address!("0x3A91A31cB3dC49b4db9Ce721F50a9D076c8D739B"),
        AVALANCHE => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        BNB => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        OPTIMISM => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
        POLYGON => address!("0x40C57923924B5c5c5455c48D93317139ADDaC8fb"),
    }
);

crate::bindings!(
    IZeroex,
    crate::deployments! {
        MAINNET => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        SEPOLIA => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        ARBITRUM_ONE => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        BASE => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        AVALANCHE => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        BNB => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        OPTIMISM => address!("0xdef1abe32c034e558cdd535791643c58a13acc10"),
        POLYGON => address!("0xdef1c0ded9bec7f1a1670819833240f027b25eff"),
        // Not available on Lens
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

    /// Returns the block number at which the contract was deployed, if known.
    fn deployed_block(
        &self,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<u64>>> + Send;
}

/// Build a `HashMap<u64, (Address, Option<u64>)>` from entries like:
/// `CHAIN_ID => address!("0x…")`                // block = None
/// `CHAIN_ID => (address!("0x…"), 12_345_678)`  // block = Some(…)
#[macro_export]
macro_rules! deployments {
    (@acc $m:ident; ) => {};

    // Tuple form with trailing comma: CHAIN => (addr, block),
    (@acc $m:ident; $chain:expr => ( $addr:expr, $block:expr ), $($rest:tt)* ) => {
        $m.insert($chain, ($addr, Some($block)));
        $crate::deployments!(@acc $m; $($rest)*);
    };

    // Address-only form with trailing comma: CHAIN => addr,
    (@acc $m:ident; $chain:expr => $addr:expr, $($rest:tt)* ) => {
        $m.insert($chain, ($addr, None::<u64>));
        $crate::deployments!(@acc $m; $($rest)*);
    };

    // Tuple form without trailing comma (last entry).
    (@acc $m:ident; $chain:expr => ( $addr:expr, $block:expr ) ) => {
        $m.insert($chain, ($addr, Some($block)));
    };

    // Address-only form without trailing comma (last entry).
    (@acc $m:ident; $chain:expr => $addr:expr ) => {
        $m.insert($chain, ($addr, None::<u64>));
    };

    ( $($rest:tt)* ) => {{
        let mut m = ::std::collections::HashMap::new();
        $crate::deployments!(@acc m; $($rest)*);
        m
    }};
}

#[macro_export]
macro_rules! bindings {
    ($contract:ident $(, $deployment_info:expr)?) => {
        paste::paste! {
            // Generate the main bindings in a private module. That allows
            // us to re-export all items in our own module while also adding
            // some items ourselves.
            #[allow(non_snake_case)]
            mod [<$contract Private>] {
                alloy::sol!(
                    #[allow(missing_docs, clippy::too_many_arguments)]
                    #[sol(rpc)]
                    $contract,
                    concat!("./artifacts/", stringify!($contract), ".json"),
                );
            }

            #[allow(non_snake_case)]
            pub mod $contract {
                use {
                    std::sync::LazyLock,
                    anyhow::{anyhow, Result},
                    alloy::{
                        json_abi::{ContractObject, JsonAbi},
                        primitives::Selector,
                        providers::DynProvider,
                    },
                };

                pub use super::[<$contract Private>]::*;
                pub type Instance = $contract::[<$contract Instance>]<DynProvider>;

                /// The contract's ABI parsed from the bundled artifact.
                pub static ABI: LazyLock<JsonAbi> = LazyLock::new(|| {
                    let obj: ContractObject = serde_json::from_str(include_str!(concat!(
                        "../artifacts/", stringify!($contract), ".json"
                    )))
                    .expect(concat!("failed to parse artifact JSON for ", stringify!($contract)));
                    obj.abi.expect(&format!("artifact for {} missing `abi` field", stringify!($contract)))
                });

                /// Return all function overloads 4-byte selectors by *name*.
                pub fn selector_by_name(name: &str) -> Result<Vec<Selector>> {
                    let Some(funcs) = ABI.functions.get(name) else {
                        return Err(anyhow!("no function named `{name}` in ABI"));
                    };
                    Ok(funcs.iter().map(|f| f.selector()).collect())
                }

                $(
                use {
                    std::collections::HashMap,
                    alloy::{
                        providers::Provider,
                        primitives::{address, Address},
                    },
                    anyhow::Context,
                    $crate::alloy::networks::*,
                };

                pub static DEPLOYMENT_INFO: LazyLock<HashMap<u64, (Address, Option<u64>)>> = LazyLock::new(|| {
                    $deployment_info
                });

                impl $crate::alloy::InstanceExt for Instance {
                    fn deployed(provider: &DynProvider) -> impl Future<Output = Result<Self>> + Send {
                        async move {
                            let chain_id = provider
                                .get_chain_id()
                                .await
                                .context("could not fetch current chain id")?;

                            let (address, _deployed_block) = *DEPLOYMENT_INFO
                                .get(&chain_id)
                                .with_context(|| format!("no deployment info for chain {chain_id:?}"))?;

                            Ok(Instance::new(
                                address,
                                provider.clone(),
                            ))
                        }
                    }

                    fn deployed_block(&self) -> impl Future<Output = Result<Option<u64>>> + Send {
                        async move {
                            let chain_id = self
                                .provider()
                                .get_chain_id()
                                .await
                                .context("could not fetch current chain id")?;
                            if let Some((_address, deployed_block)) = DEPLOYMENT_INFO.get(&chain_id) {
                                return Ok(*deployed_block);
                            }
                            Ok(None)
                        }
                    }
                }
                )*
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::Selector;

    #[test]
    fn test_selector_by_name_valid_function() {
        let result = ChainalysisOracle::selector_by_name("isSanctioned");
        assert!(result.is_ok());

        let selectors = result.unwrap();
        assert_eq!(selectors.len(), 1);

        let selector = &selectors[0];
        assert_eq!(selector, &Selector::from([0xdf, 0x59, 0x2f, 0x7d]));
    }

    #[test]
    fn test_selector_by_name_multiple_overloads() {
        // Test with a contract that might have function overloads
        // Using IZeroex which likely has multiple swap functions
        let result = IZeroex::selector_by_name("transformERC20");

        let selectors = result.unwrap();

        assert!(!selectors.is_empty());

        for selector in &selectors {
            assert_eq!(selector.as_slice().len(), 4);
        }
    }

    #[test]
    fn test_selector_by_name_invalid_function() {
        let result = ChainalysisOracle::selector_by_name("nonExistentFunction");
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("no function named `nonExistentFunction` in ABI"));
    }

    #[test]
    fn test_selector_by_name_empty_string() {
        let result = ChainalysisOracle::selector_by_name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_selector_by_name_case_sensitive() {
        let result1 = ChainalysisOracle::selector_by_name("isSanctioned");
        let result2 = ChainalysisOracle::selector_by_name("IsSanctioned");
        let result3 = ChainalysisOracle::selector_by_name("ISSANCTIONED");

        assert!(result1.is_ok());
        assert!(result2.is_err());
        assert!(result3.is_err());
    }
}
