use {
    crate::{
        domain::eth,
        infra::{blockchain, Ethereum},
    },
    ethrpc::current_block::{self, CurrentBlockStream},
    futures::StreamExt,
    std::{
        collections::{HashMap, HashSet},
        sync::{Arc, RwLock},
    },
    tracing::Instrument,
};

#[derive(Clone, Debug)]
pub struct Metadata {
    pub decimals: Option<u8>,
    pub symbol: Option<String>,
    /// Current balance of the smart contract.
    pub balance: eth::TokenAmount,
}

#[derive(Clone)]
pub struct Fetcher(Arc<Inner>);

impl Fetcher {
    pub fn new(eth: Ethereum) -> Self {
        let block_stream = eth.current_block().clone();
        let inner = Arc::new(Inner {
            eth,
            cache: RwLock::new(HashMap::new()),
        });
        tokio::task::spawn(
            update_task(block_stream, Arc::downgrade(&inner))
                .instrument(tracing::info_span!("token_fetcher")),
        );
        Self(inner)
    }

    /// Returns the `Metadata` for the given tokens. Note that the result will
    /// not contain data for tokens that encountered errors while fetching
    /// the data.
    pub async fn get(
        &self,
        addresses: &[eth::TokenAddress],
    ) -> HashMap<eth::TokenAddress, Metadata> {
        self.0.get(addresses).await
    }
}

async fn update_task(blocks: CurrentBlockStream, inner: std::sync::Weak<Inner>) {
    let mut stream = current_block::into_stream(blocks);
    while stream.next().await.is_some() {
        let inner = match inner.upgrade() {
            Some(inner) => inner,
            // Fetcher was dropped, stop update task.
            None => break,
        };
        if let Err(err) = update_cache(inner).await {
            tracing::warn!(?err, "error updating token cache");
        }
    }
}

async fn update_cache(inner: Arc<Inner>) -> Result<(), blockchain::Error> {
    let settlement = inner.eth.contracts().settlement().address().into();
    let futures = {
        let cache = inner.cache.read().unwrap();
        let tokens = cache.keys().cloned().collect::<HashSet<_>>();
        tracing::debug!(
            tokens = tokens.len(),
            "updating settlement contract balances"
        );
        tokens.into_iter().map(|token| {
            let erc20 = inner.eth.erc20(token);
            async move {
                Ok::<(eth::TokenAddress, eth::TokenAmount), blockchain::Error>((
                    token,
                    erc20.balance(settlement).await?,
                ))
            }
        })
    };

    // Don't hold on to the lock while fetching balances. This may lead to new
    // entries arriving in the meantime, however their balances should be
    // up-to-date.
    let balances = futures::future::join_all(futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let mut cache: std::sync::RwLockWriteGuard<'_, HashMap<eth::TokenAddress, Metadata>> =
        inner.cache.write().unwrap();
    for (key, entry) in cache.iter_mut() {
        if let Some(balance) = balances.get(key) {
            entry.balance = balance.clone();
        } else {
            tracing::info!(?key, "key without balance update");
        }
    }
    Ok(())
}

/// Provides metadata of tokens.
struct Inner {
    eth: Ethereum,
    cache: RwLock<HashMap<eth::TokenAddress, Metadata>>,
}

impl Inner {
    /// Fetches `Metadata` of the requested tokens from a node.
    async fn fetch_token_infos(
        &self,
        tokens: HashSet<eth::TokenAddress>,
    ) -> Vec<Result<(eth::TokenAddress, Metadata), blockchain::Error>> {
        let settlement = self.eth.contracts().settlement().address().into();
        let futures = tokens.into_iter().map(|token| async move {
            let token = self.eth.erc20(token);
            // Use `try_join` because these calls get batched under the hood
            // so if one of them fails the others will as well.
            // Also this way we won't get incomplete data for a token.
            let (decimals, symbol, balance) = futures::future::try_join3(
                token.decimals(),
                token.symbol(),
                token.balance(settlement),
            )
            .await?;
            Ok((
                token.address(),
                Metadata {
                    decimals,
                    symbol,
                    balance,
                },
            ))
        });
        futures::future::join_all(futures).await
    }

    async fn get(&self, addresses: &[eth::TokenAddress]) -> HashMap<eth::TokenAddress, Metadata> {
        let to_fetch: HashSet<_> = {
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
            let fetched = self.fetch_token_infos(to_fetch).await;

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
