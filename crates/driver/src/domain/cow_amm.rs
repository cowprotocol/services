use {
    crate::domain::eth,
    contracts::CowAmmLegacyHelper,
    cow_amm::Amm,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    itertools::{
        Either::{Left, Right},
        Itertools,
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tokio::sync::RwLock,
};

/// Cache for CoW AMM data to avoid using the registry dependency.
/// Maps AMM address to the corresponding Amm instance.
pub struct Cache {
    inner: RwLock<HashMap<eth::Address, Arc<Amm>>>,
    web3: ethrpc::Web3,
    helper_by_factory: HashMap<eth::Address, CowAmmLegacyHelper>,
}

impl Cache {
    pub fn new(web3: ethrpc::Web3, factory_mapping: HashMap<eth::Address, eth::Address>) -> Self {
        let helper_by_factory = factory_mapping
            .into_iter()
            .map(|(factory, helper)| (factory, CowAmmLegacyHelper::at(&web3, helper.0)))
            .collect();
        Self {
            inner: RwLock::new(HashMap::new()),
            web3,
            helper_by_factory,
        }
    }

    /// Gets or creates AMM instances for the given surplus capturing JIT order
    /// owners.
    pub async fn get_or_create_amms(
        &self,
        surplus_capturing_jit_order_owners: &HashSet<eth::Address>,
    ) -> Vec<Arc<Amm>> {
        let (mut cached_amms, missing_amms): (Vec<Arc<Amm>>, Vec<eth::Address>) = {
            let cache = self.inner.read().await;
            surplus_capturing_jit_order_owners
                .iter()
                .partition_map(|&address| match cache.get(&address) {
                    Some(amm) => Left(amm.clone()),
                    None => Right(address),
                })
        };

        if missing_amms.is_empty() {
            return cached_amms;
        }

        let fetch_futures = missing_amms.into_iter().map(|amm_address| async move {
            let factory_address = self
                .fetch_amm_factory_address(amm_address)
                .await
                .inspect_err(|err| {
                    tracing::warn!(
                        ?err,
                        amm_address = ?amm_address.0,
                        "failed to fetch CoW AMM factory address"
                    );
                })
                .ok()?;

            let Some(helper) = self.helper_by_factory.get(&factory_address) else {
                tracing::warn!(
                    factory_address = ?factory_address.0,
                    amm_address = ?amm_address.0,
                    "no helper contract configured for CoW AMM factory"
                );
                return None;
            };

            match Amm::new(amm_address.0, helper).await {
                Ok(amm) => Some((amm_address, Arc::new(amm))),
                Err(err) => {
                    let helper_address = helper.address().0;
                    tracing::warn!(
                        ?err,
                        amm_address = ?amm_address.0,
                        ?helper_address,
                        "failed to create CoW AMM instance"
                    );
                    None
                }
            }
        });

        let fetched_results = futures::future::join_all(fetch_futures).await;

        // Update cache with newly fetched AMMs
        let newly_created_amms = {
            let mut cache = self.inner.write().await;
            fetched_results
                .into_iter()
                .flatten()
                .map(|(amm_address, amm)| {
                    cache.insert(amm_address, amm.clone());
                    amm
                })
                .collect::<Vec<_>>()
        };

        // Combine cached and newly created AMMs
        cached_amms.extend(newly_created_amms);
        cached_amms
    }

    /// Fetches the factory address for the given AMM by calling the
    /// `FACTORY` function.
    async fn fetch_amm_factory_address(
        &self,
        amm_address: eth::Address,
    ) -> anyhow::Result<eth::Address> {
        let factory_getter =
            contracts::alloy::cow_amm::CowAmmFactoryGetter::CowAmmFactoryGetter::new(
                amm_address.0.into_alloy(),
                &self.web3.alloy,
            );
        Ok(factory_getter.FACTORY().call().await?.into_legacy().into())
    }
}
