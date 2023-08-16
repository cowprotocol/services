use {
    crate::{domain::eth, infra::Ethereum},
    anyhow::Result,
    contracts::ERC20,
    ethcontract::dyns::DynWeb3,
    std::{
        collections::HashMap,
        sync::{Arc, RwLock},
    },
};

#[derive(Clone, Debug, Default)]
pub struct Metadata {
    pub decimals: u8,
    pub symbol: String,
}

/// Provides metadata of tokens.
pub struct Fetcher {
    web3: DynWeb3,
    cache: Arc<RwLock<HashMap<eth::TokenAddress, Metadata>>>,
}

impl Fetcher {
    pub fn new(eth: &Ethereum) -> Self {
        Self {
            web3: eth.web3().clone(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Fetches metadata of the requested token from a node.
    async fn fetch_token_infos(
        &self,
        addresses: &[eth::TokenAddress],
    ) -> Vec<Result<(eth::TokenAddress, Metadata)>> {
        let futures = addresses.iter().map(|address| async {
            let erc20 = ERC20::at(&self.web3, address.0 .0);
            // Use `try_join` because these calls get batched under the hood
            // so if one of them fails the other will as well.
            // Also this way we won't get incomplete data for a token.
            let (decimals, symbol) = futures::future::try_join(
                erc20.methods().decimals().call(),
                erc20.methods().symbol().call(),
            )
            .await?;
            Ok((*address, Metadata { decimals, symbol }))
        });
        futures::future::join_all(futures).await
    }

    /// Returns the `Metadata` for the given tokens. Note that the result will
    /// not contain data for tokens that encountered errors while fetching
    /// the data.
    pub async fn get_token_infos(
        &self,
        addresses: &[eth::TokenAddress],
    ) -> HashMap<eth::TokenAddress, Metadata> {
        let to_fetch: Vec<_> = {
            let cache = self.cache.read().unwrap();

            // Compute set of requested addresses that are not in cache.
            addresses
                .iter()
                .filter(|address| !cache.contains_key(*address))
                .cloned()
                .collect()
        };

        // Fetch token infos not yet in cache.
        if !to_fetch.is_empty() {
            let fetched = self.fetch_token_infos(to_fetch.as_slice()).await;

            // Add valid token infos to cache.
            self.cache
                .write()
                .unwrap()
                .extend(fetched.into_iter().flatten());
        };

        let cache = self.cache.read().unwrap();
        // Return token infos from the cache.
        addresses
            .iter()
            .filter_map(|address| Some((*address, cache.get(address)?.clone())))
            .collect()
    }
}
