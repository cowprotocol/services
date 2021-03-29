use async_trait::async_trait;
use contracts::ERC20;
use ethcontract::{batch::CallBatch, Http, Web3, H160};
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use mockall::*;

const MAX_BATCH_SIZE: usize = 100;

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Copy, Clone, Debug)]
pub struct TokenInfo {
    pub decimals: Option<u8>,
}

pub struct TokenInfoFetcher {
    pub web3: Web3<Http>,
}

#[automock]
#[async_trait]
pub trait TokenInfoFetching: Send + Sync {
    /// Retrieves all token information.
    /// Default implementation calls get_token_info for each token and ignores errors.
    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo>;
}

#[async_trait]
impl TokenInfoFetching for TokenInfoFetcher {
    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo> {
        let web3 = Web3::new(self.web3.transport().clone());
        let mut batch = CallBatch::new(self.web3.transport());
        let futures = addresses
            .iter()
            .map(|address| {
                let erc20 = ERC20::at(&web3, *address);
                erc20.methods().decimals().batch_call(&mut batch)
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;

        addresses
            .iter()
            .zip(join_all(futures).await.into_iter())
            .map(|(address, decimals)| {
                if decimals.is_err() {
                    tracing::trace!("Failed to fetch token info for token {}", address);
                }
                (
                    *address,
                    TokenInfo {
                        decimals: decimals.ok(),
                    },
                )
            })
            .collect()
    }
}

pub struct CachedTokenInfoFetcher {
    inner: Box<dyn TokenInfoFetching>,
    cache: Arc<Mutex<HashMap<H160, TokenInfo>>>,
}

impl CachedTokenInfoFetcher {
    pub fn new(inner: Box<dyn TokenInfoFetching>) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl TokenInfoFetching for CachedTokenInfoFetcher {
    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo> {
        let mut cache = self.cache.lock().await;

        // Compute set of requested addresses that are not in cache.
        let to_fetch: Vec<H160> = addresses
            .iter()
            .filter(|address| !cache.contains_key(address))
            .cloned()
            .collect();

        // Fetch token infos not yet in cache.
        if !to_fetch.is_empty() {
            let fetched = self.inner.get_token_infos(to_fetch.as_slice()).await;

            // Add valid token infos to cache.
            cache.extend(
                fetched
                    .iter()
                    .filter(|(_, token_info)| token_info.decimals.is_some()),
            );
        };

        // Return token infos from the cache.
        addresses
            .iter()
            .map(|address| {
                if cache.contains_key(address) {
                    (*address, cache[address])
                } else {
                    (*address, TokenInfo { decimals: None })
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;

    #[tokio::test]
    async fn cached_token_info_fetcher() {
        let address0 = H160::zero();
        let address1 = H160::from_low_u64_be(1);

        let mut mock_token_info_fetcher = MockTokenInfoFetching::new();
        mock_token_info_fetcher
            .expect_get_token_infos()
            .times(1)
            .return_once(move |_| {
                hashmap! {
                    address0 => TokenInfo { decimals: Some(18)},
                }
            });
        mock_token_info_fetcher
            .expect_get_token_infos()
            .times(2)
            .returning(|_| {
                hashmap! {
                    H160::from_low_u64_be(1) => TokenInfo { decimals: None},
                }
            });
        let cached_token_info_fetcher =
            CachedTokenInfoFetcher::new(Box::new(mock_token_info_fetcher));

        // Fetching a cached item should work.
        let token_infos = cached_token_info_fetcher.get_token_infos(&[address0]).await;
        assert!(token_infos.contains_key(&address0) && token_infos[&address0].decimals == Some(18));

        // Should panic because of the times(1) constraint above, unless the cache is working as expected.
        cached_token_info_fetcher.get_token_infos(&[address0]).await;

        // Fetching an item that is unavailable should work.
        let token_infos = cached_token_info_fetcher.get_token_infos(&[address1]).await;
        assert!(token_infos.contains_key(&address1) && token_infos[&address1].decimals == None);

        // Should try to refetch the item thus satisfying the times(2) constraint above.
        cached_token_info_fetcher.get_token_infos(&[address1]).await;
    }
}
