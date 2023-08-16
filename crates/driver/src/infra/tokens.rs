use {
    crate::{domain::eth, infra::Ethereum},
    anyhow::Result,
    std::{collections::HashMap, sync::RwLock},
};

#[derive(Clone, Debug, Default)]
pub struct Metadata {
    pub decimals: u8,
    pub symbol: String,
}

/// Provides metadata of tokens.
pub struct Fetcher {
    eth: Ethereum,
    cache: RwLock<HashMap<eth::TokenAddress, Metadata>>,
}

impl Fetcher {
    pub fn new(eth: Ethereum) -> Self {
        Self {
            eth,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Fetches `Metadata` of the requested tokens from a node.
    async fn fetch_token_infos(
        &self,
        addresses: &[eth::TokenAddress],
    ) -> Vec<Result<(eth::TokenAddress, Metadata)>> {
        let futures = addresses.iter().map(|address| async {
            // Use `try_join` because these calls get batched under the hood
            // so if one of them fails the other will as well.
            // Also this way we won't get incomplete data for a token.
            let (decimals, symbol) =
                futures::future::try_join(self.eth.decimals(*address), self.eth.symbol(*address))
                    .await?;
            Ok((*address, Metadata { decimals, symbol }))
        });
        futures::future::join_all(futures).await
    }

    /// Returns the `Metadata` for the given tokens. Note that the result will
    /// not contain data for tokens that encountered errors while fetching
    /// the data.
    pub async fn get(
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
