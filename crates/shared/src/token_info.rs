use {
    anyhow::Result,
    async_trait::async_trait,
    contracts::{errors::EthcontractErrorType, ERC20},
    ethcontract::{errors::MethodError, H160},
    ethrpc::Web3,
    futures::{
        future::{BoxFuture, Shared},
        FutureExt,
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

#[mockall::automock]
#[async_trait]
pub trait TokenInfoFetching: Send + Sync {
    /// Retrieves information for a token.
    async fn get_token_info(&self, address: H160) -> Result<TokenInfo, Error>;

    /// Retrieves all token information.
    /// Default implementation calls get_token_info for each token and ignores
    /// errors.
    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo>;
}

pub struct TokenInfoFetcher {
    pub web3: Web3,
}

impl TokenInfoFetcher {
    async fn fetch_token(&self, address: H160) -> Result<TokenInfo, Error> {
        if address == BUY_ETH_ADDRESS {
            return Ok(TokenInfo {
                decimals: Some(18),
                symbol: Some("NATIVE_ASSET".to_string()),
            });
        }

        let erc20 = ERC20::at(&self.web3, address);
        let (decimals, symbol) = futures::join!(
            erc20.methods().decimals().call(),
            erc20.methods().symbol().call(),
        );

        Ok(TokenInfo {
            decimals: classify_error(decimals)?,
            symbol: classify_error(symbol)?,
        })
    }
}

fn classify_error<T>(result: Result<T, MethodError>) -> Result<Option<T>, Error> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(err) => match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Node => Err(Error(err.to_string())),
            EthcontractErrorType::Contract => Ok(None),
        },
    }
}

#[async_trait]
impl TokenInfoFetching for TokenInfoFetcher {
    async fn get_token_info(&self, address: H160) -> Result<TokenInfo, Error> {
        let info = self.fetch_token(address).await;
        if let Err(err) = &info {
            tracing::debug!(?err, token = ?address, "failed to fetch token info");
        }

        info
    }

    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo> {
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
    cache: Arc<Mutex<HashMap<H160, SharedTokenInfo>>>,
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
    async fn fetch_token(&self, address: H160) -> Result<TokenInfo, Error> {
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
    async fn get_token_info(&self, address: H160) -> Result<TokenInfo, Error> {
        self.fetch_token(address).await
    }

    async fn get_token_infos(&self, addresses: &[H160]) -> HashMap<H160, TokenInfo> {
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
        let address = H160::from_low_u64_be;

        let mut mock_token_info_fetcher = MockTokenInfoFetching::new();
        mock_token_info_fetcher
            .expect_get_token_info()
            .with(eq(address(0)))
            .times(1)
            .return_once(move |_| {
                Ok(TokenInfo {
                    decimals: Some(18),
                    symbol: Some("CAT".to_string()),
                })
            });
        mock_token_info_fetcher
            .expect_get_token_info()
            .with(eq(address(1)))
            .times(1)
            .return_once(move |_| {
                Ok(TokenInfo {
                    decimals: None,
                    symbol: None,
                })
            });
        mock_token_info_fetcher
            .expect_get_token_info()
            .with(eq(address(2)))
            .times(2)
            .returning(|_| Err(Error("some error".to_string())));

        let cached_token_info_fetcher =
            CachedTokenInfoFetcher::new(Arc::new(mock_token_info_fetcher));

        // Fetches tokens, using `TokenInfo::default()` for the failed token.
        let addresses = [address(0), address(1), address(2)];
        let token_infos = cached_token_info_fetcher.get_token_infos(&addresses).await;
        assert_eq!(
            token_infos,
            hashmap! {
                address(0) => TokenInfo {
                    decimals: Some(18),
                    symbol: Some("CAT".to_string()),
                },
                address(1) => TokenInfo {
                    decimals: None,
                    symbol: None,
                },
                address(2) => TokenInfo::default(),
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
