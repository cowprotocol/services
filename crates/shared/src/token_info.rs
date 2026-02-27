use {
    alloy::primitives::Address,
    anyhow::Result,
    async_trait::async_trait,
    contracts::ERC20,
    ethrpc::{Web3, alloy::errors::ignore_non_node_error},
    futures::{
        FutureExt,
        future::{BoxFuture, Shared},
    },
    model::order::BUY_ETH_ADDRESS,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
    thiserror::Error,
};

#[cfg_attr(test, derive(Eq, PartialEq))]
#[derive(Clone, Debug, Default)]
pub struct TokenInfo {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
}

#[derive(Clone, Debug, Error)]
#[error("error fetching token info: {0}")]
pub struct Error(String);

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait]
pub trait TokenInfoFetching: Send + Sync {
    /// Retrieves information for a token.
    async fn get_token_info(&self, address: Address) -> Result<TokenInfo, Error>;

    /// Retrieves all token information.
    /// Default implementation calls get_token_info for each token and ignores
    /// errors.
    async fn get_token_infos(&self, addresses: &[Address]) -> HashMap<Address, TokenInfo>;
}

pub struct TokenInfoFetcher {
    pub web3: Web3,
}

impl TokenInfoFetcher {
    async fn fetch_token(&self, address: Address) -> Result<TokenInfo, Error> {
        if address == BUY_ETH_ADDRESS {
            return Ok(TokenInfo {
                decimals: Some(18),
                symbol: Some("NATIVE_ASSET".to_string()),
            });
        }

        let erc20 = ERC20::Instance::new(address, self.web3.provider.clone());
        let (decimals, symbol) = {
            let decimals = erc20.decimals();
            let symbol = erc20.symbol();
            futures::join!(decimals.call().into_future(), symbol.call().into_future())
        };

        Ok(TokenInfo {
            decimals: ignore_non_node_error(decimals).map_err(|err| Error(err.to_string()))?,
            symbol: ignore_non_node_error(symbol).map_err(|err| Error(err.to_string()))?,
        })
    }
}

#[async_trait]
impl TokenInfoFetching for TokenInfoFetcher {
    async fn get_token_info(&self, address: Address) -> Result<TokenInfo, Error> {
        let info = self.fetch_token(address).await;
        if let Err(err) = &info {
            tracing::debug!(?err, token = ?address, "failed to fetch token info");
        }

        info
    }

    async fn get_token_infos(&self, addresses: &[Address]) -> HashMap<Address, TokenInfo> {
        futures::future::join_all(addresses.iter().copied().map(|address| async move {
            let info = self.fetch_token(address).await;
            if let Err(err) = &info {
                tracing::debug!(?err, token = ?address, "failed to fetch token info");
            }

            (address, info.unwrap_or_default())
        }))
        .await
        .into_iter()
        .collect()
    }
}

type SharedTokenInfo = Shared<BoxFuture<'static, Result<TokenInfo, Error>>>;

pub struct CachedTokenInfoFetcher {
    inner: Arc<dyn TokenInfoFetching>,
    cache: Arc<Mutex<HashMap<Address, SharedTokenInfo>>>,
}

impl CachedTokenInfoFetcher {
    pub fn new(inner: Arc<dyn TokenInfoFetching>) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl CachedTokenInfoFetcher {
    async fn fetch_token(&self, address: Address) -> Result<TokenInfo, Error> {
        let fetch = {
            let mut cache = self.cache.lock().unwrap();
            cache
                .entry(address)
                .or_insert({
                    let inner = self.inner.clone();
                    async move { inner.get_token_info(address).await }
                        .boxed()
                        .shared()
                })
                .clone()
        };

        let info = fetch.await;
        if info.is_err() {
            let mut cache = self.cache.lock().unwrap();
            if let Some(Err(_)) = cache.get(&address).and_then(|fetch| fetch.peek()) {
                cache.remove(&address);
            }
        }

        info
    }
}

#[async_trait]
impl TokenInfoFetching for CachedTokenInfoFetcher {
    async fn get_token_info(&self, address: Address) -> Result<TokenInfo, Error> {
        self.fetch_token(address).await
    }

    async fn get_token_infos(&self, addresses: &[Address]) -> HashMap<Address, TokenInfo> {
        futures::future::join_all(addresses.iter().copied().map(|address| async move {
            (
                address,
                self.get_token_info(address).await.unwrap_or_default(),
            )
        }))
        .await
        .into_iter()
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, maplit::hashmap, mockall::predicate::*};

    #[tokio::test]
    async fn cached_token_info_fetcher() {
        let mut mock_token_info_fetcher = MockTokenInfoFetching::new();
        mock_token_info_fetcher
            .expect_get_token_info()
            .with(eq(Address::with_last_byte(0)))
            .times(1)
            .return_once(move |_| {
                Ok(TokenInfo {
                    decimals: Some(18),
                    symbol: Some("CAT".to_string()),
                })
            });
        mock_token_info_fetcher
            .expect_get_token_info()
            .with(eq(Address::with_last_byte(1)))
            .times(1)
            .return_once(move |_| {
                Ok(TokenInfo {
                    decimals: None,
                    symbol: None,
                })
            });
        mock_token_info_fetcher
            .expect_get_token_info()
            .with(eq(Address::with_last_byte(2)))
            .times(2)
            .returning(|_| Err(Error("some error".to_string())));

        let cached_token_info_fetcher =
            CachedTokenInfoFetcher::new(Arc::new(mock_token_info_fetcher));

        // Fetches tokens, using `TokenInfo::default()` for the failed token.
        let addresses = [
            Address::with_last_byte(0),
            Address::with_last_byte(1),
            Address::with_last_byte(2),
        ];
        let token_infos = cached_token_info_fetcher.get_token_infos(&addresses).await;
        assert_eq!(
            token_infos,
            hashmap! {
                Address::with_last_byte(0) => TokenInfo {
                    decimals: Some(18),
                    symbol: Some("CAT".to_string()),
                },
                Address::with_last_byte(1) => TokenInfo {
                    decimals: None,
                    symbol: None,
                },
                Address::with_last_byte(2) => TokenInfo::default(),
            }
        );

        // Fetch again, if the the two token 0 and 1 are fetched again (i.e. the
        // cache is not working) then this will panic because of the `times(1)`
        // constraint on our mock fetcher. Note that token 2 gets fetched again
        // because it failed to fetch the first time.
        let cached_token_infos = cached_token_info_fetcher.get_token_infos(&addresses).await;
        assert_eq!(token_infos, cached_token_infos);
    }
}
