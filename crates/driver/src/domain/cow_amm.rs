use {
    crate::domain::eth,
    contracts::CowAmmLegacyHelper,
    cow_amm::Amm,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::RwLock,
};

/// Cache for CoW AMM data to avoid using the registry dependency.
/// Maps AMM address to the corresponding Amm instance.
pub struct Cache {
    inner: RwLock<HashMap<eth::Address, Arc<Amm>>>,
    web3: ethrpc::Web3,
}

impl Cache {
    pub fn new(web3: ethrpc::Web3) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            web3,
        }
    }

    /// Gets or creates AMM instances for the given surplus capturing JIT order
    /// owners with their helpers. Returns a list of AMM instances that were
    /// successfully created or retrieved from cache.
    pub async fn get_or_create_amms(
        &self,
        surplus_capturing_jit_order_owners_with_helper: &HashMap<eth::Address, eth::Address>,
    ) -> Vec<Arc<Amm>> {
        let mut cached_amms = Vec::new();
        let mut missing_amms = Vec::new();

        {
            let cache = self.inner.read().await;
            for (amm_address, helper_address) in surplus_capturing_jit_order_owners_with_helper {
                if let Some(amm) = cache.get(amm_address) {
                    cached_amms.push(amm.clone());
                } else {
                    missing_amms.push((*amm_address, *helper_address));
                }
            }
        }

        if missing_amms.is_empty() {
            return cached_amms;
        }

        let fetch_futures = missing_amms
            .into_iter()
            .map(|(amm_address, helper_address)| {
                let web3 = self.web3.clone();
                async move {
                    let helper = CowAmmLegacyHelper::at(&web3, helper_address.0);
                    match Amm::new(amm_address.0, &helper).await {
                        Ok(amm) => Some((amm_address, Arc::new(amm))),
                        Err(err) => {
                            tracing::warn!(
                                ?err,
                                amm_address = ?amm_address.0,
                                helper_address = ?helper_address.0,
                                "failed to create CoW AMM instance"
                            );
                            None
                        }
                    }
                }
            });

        let fetched_results = futures::future::join_all(fetch_futures).await;

        // Update cache with newly fetched AMMs
        let mut newly_created_amms = Vec::new();
        {
            let mut cache = self.inner.write().await;
            for (amm_address, amm) in fetched_results.into_iter().flatten() {
                cache.insert(amm_address, amm.clone());
                newly_created_amms.push(amm);
            }
        }

        // Combine cached and newly created AMMs
        cached_amms.extend(newly_created_amms);
        cached_amms
    }
}
