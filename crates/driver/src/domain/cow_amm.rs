use {
    crate::domain::eth,
    contracts::CowAmmLegacyHelper,
    cow_amm::Amm,
    hex_literal::hex,
    itertools::{
        Either::{Left, Right},
        Itertools,
    },
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    tokio::sync::RwLock,
    web3::types::{Bytes, CallRequest},
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

        let fetch_futures = missing_amms.into_iter().map(|amm_address| {
            let web3 = self.web3.clone();
            async move {
                let helper_address = match self.fetch_amm_factory_address(amm_address).await {
                    Ok(address) => address,
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            amm_address = ?amm_address.0,
                            "failed to fetch CoW AMM factory address"
                        );
                        return None;
                    }
                };

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

    async fn fetch_amm_factory_address(
        &self,
        amm_address: eth::Address,
    ) -> anyhow::Result<eth::Address> {
        const FUNCTION_SELECTOR: [u8; 4] = hex!("2dd31000");
        const FUNCTION_SELECTOR_LEGACY: [u8; 4] = hex!("c45a0155");

        let req = CallRequest::builder()
            .to(amm_address.0)
            .data(Bytes(FUNCTION_SELECTOR.to_vec()))
            .build();

        let address = match self.web3.eth().call(req, None).await {
            Ok(address) => Ok(address),
            Err(_) => {
                let req_legacy = CallRequest::builder()
                    .to(amm_address.0)
                    .data(Bytes(FUNCTION_SELECTOR_LEGACY.to_vec()))
                    .build();
                self.web3.eth().call(req_legacy, None).await
            }
        };

        Ok(eth::Address(eth::H160::from_slice(&address?.0)))
    }
}
