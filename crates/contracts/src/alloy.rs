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
