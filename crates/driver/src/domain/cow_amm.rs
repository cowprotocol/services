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
            let factory_address = match self.fetch_amm_factory_address(amm_address).await {
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

            let helper = match self.helper_by_factory.get(&factory_address) {
                Some(contract) => contract,
                None => {
                    tracing::warn!(
                        factory_address = ?factory_address.0,
                        amm_address = ?amm_address.0,
                        "no helper contract configured for CoW AMM factory"
                    );
                    return None;
                }
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

    /// Fetches the factory address for the given AMM by calling the
    /// `factory` function. If that fails, it tries the legacy function
    /// `FACTORY`.
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
            Ok(result) => Self::parse_address(result),
            Err(_) => {
                let req_legacy = CallRequest::builder()
                    .to(amm_address.0)
                    .data(Bytes(FUNCTION_SELECTOR_LEGACY.to_vec()))
                    .build();
                let result = self.web3.eth().call(req_legacy, None).await?;
                Self::parse_address(result)
            }
        };

        Ok(address?)
    }

    fn parse_address(data: Bytes) -> anyhow::Result<eth::Address> {
        match data.0.len() {
            32 => Ok(eth::Address(eth::H160::from_slice(&data.0[12..]))),
            20 => Ok(eth::Address(eth::H160::from_slice(&data.0))),
            invalid => Err(anyhow::anyhow!("Invalid address length: {}", invalid)),
        }
    }
}
