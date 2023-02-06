use {
    crate::ethrpc::Web3,
    async_trait::async_trait,
    contracts::ERC20,
    ethcontract::{batch::CallBatch, H160},
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Mutex,
};

const MAX_BATCH_SIZE: usize = 100;

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Clone, Debug, Default)]
pub struct TokenInfo {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
}

pub struct TokenInfoFetcher {
    pub web3: Web3,
}

#[mockall::automock]
#[async_trait]
pub trait TokenInfoFetching: Send + Sync {
    /// Retrieves all token information.
    /// Default implementation calls get_token_info for each token and ignores
    /// errors.
    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo>;
}

#[async_trait]
impl TokenInfoFetching for TokenInfoFetcher {
    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo> {
        let mut batch = CallBatch::new(self.web3.transport());
        let futures = addresses
            .iter()
            .map(|address| {
                let erc20 = ERC20::at(&self.web3, *address);
                (
                    erc20.methods().decimals().batch_call(&mut batch),
                    erc20.methods().symbol().batch_call(&mut batch),
                )
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;
        let mut resolved_futures = Vec::with_capacity(futures.len());
        for (decimals, symbol) in futures {
            resolved_futures.push((decimals.await, symbol.await));
        }
        addresses
            .iter()
            .zip(resolved_futures)
            .map(|(address, (decimals, symbol))| {
                if decimals.is_err() {
                    tracing::trace!("Failed to fetch token info for token {}", address);
                }
                (
                    *address,
                    TokenInfo {
                        decimals: decimals.ok(),
                        symbol: symbol.ok(),
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
                    .into_iter()
                    .filter(|(_, token_info)| token_info.decimals.is_some()),
            );
        };

        // Return token infos from the cache.
        addresses
            .iter()
            .map(|address| {
                if cache.contains_key(address) {
                    (*address, cache[address].clone())
                } else {
                    (
                        *address,
                        TokenInfo {
                            decimals: None,
                            symbol: None,
                        },
                    )
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, maplit::hashmap};

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
                    address0 => TokenInfo { decimals: Some(18), symbol: Some("CAT".to_string()) },
                }
            });
        mock_token_info_fetcher
            .expect_get_token_infos()
            .times(2)
            .returning(|_| {
                hashmap! {
                    H160::from_low_u64_be(1) => TokenInfo { decimals: None, symbol: None },
                }
            });
        let cached_token_info_fetcher =
            CachedTokenInfoFetcher::new(Box::new(mock_token_info_fetcher));

        // Fetching a cached item should work.
        let token_infos = cached_token_info_fetcher.get_token_infos(&[address0]).await;
        assert!(token_infos.contains_key(&address0) && token_infos[&address0].decimals == Some(18));

        // Should panic because of the times(1) constraint above, unless the cache is
        // working as expected.
        cached_token_info_fetcher.get_token_infos(&[address0]).await;

        // Fetching an item that is unavailable should work.
        let token_infos = cached_token_info_fetcher.get_token_infos(&[address1]).await;
        assert!(token_infos.contains_key(&address1) && token_infos[&address1].decimals.is_none());

        // Should try to refetch the item thus satisfying the times(2) constraint above.
        cached_token_info_fetcher.get_token_infos(&[address1]).await;
    }
}
