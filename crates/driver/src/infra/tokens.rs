use {
    crate::{
        domain::eth,
        infra::{blockchain, Ethereum},
    },
    ethrpc::current_block::{self, CurrentBlockStream},
    futures::StreamExt,
    itertools::Itertools,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::RwLock,
    tracing::Instrument,
    model::order::BUY_ETH_ADDRESS,
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

/// Runs a single cache update cycle whenever a new block arrives until the
/// fetcher is dropped.
async fn update_task(blocks: CurrentBlockStream, inner: std::sync::Weak<Inner>) {
    let mut stream = current_block::into_stream(blocks);
    while stream.next().await.is_some() {
        let inner = match inner.upgrade() {
            Some(inner) => inner,
            // Fetcher was dropped, stop update task.
            None => break,
        };
        if let Err(err) = update_balances(inner).await {
            tracing::warn!(?err, "error updating token cache");
        }
    }
}

/// Updates the settlement contract's balance for every cached token.
async fn update_balances(inner: Arc<Inner>) -> Result<(), blockchain::Error> {
    let settlement = inner.eth.contracts().settlement().address().into();
    let futures = {
        let cache = inner.cache.read().await;
        let tokens = cache.keys().cloned().collect::<Vec<_>>();
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

    tracing::debug!(
        tokens = futures.len(),
        "updating settlement contract balances"
    );

    // Don't hold on to the lock while fetching balances to allow concurrent
    // updates. This may lead to new entries arriving in the meantime, however
    // their balances should already be up-to-date.
    let mut balances = futures::future::try_join_all(futures)
        .await?
        .into_iter()
        .collect::<HashMap<_, _>>();

    let mut keys_without_balances = vec![];
    {
        let mut cache = inner.cache.write().await;
        for (key, entry) in cache.iter_mut() {
            if let Some(balance) = balances.remove(key) {
                entry.balance = balance;
            } else {
                // Avoid logging while holding the exclusive lock.
                keys_without_balances.push(*key);
            }
        }
    }
    if !keys_without_balances.is_empty() {
        tracing::info!(keys = ?keys_without_balances, "updated keys without balance");
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
        tokens: Vec<eth::TokenAddress>,
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

    /// Ensures that all the missing tokens are in the cache afterwards while
    /// taking into account that the function might be called multiple times
    /// for the same tokens.
    async fn cache_missing_tokens(&self, tokens: Vec<eth::TokenAddress>) {
        if tokens.is_empty() {
            return;
        }

        // Take exclusive lock because everybody will call this function for the same tokens
        // so this a simple way to avoid sending duplicate requests.
        let mut cache = self.cache.write().await;
        if tokens.iter().all(|address| cache.contains_key(address)) {
            // Somebody else might have already cached all the data in the meantime.
            return;
        }
        let fetched = self.fetch_token_infos(tokens).await;
        cache.extend(fetched.into_iter().flatten());
    }

    async fn get(&self, addresses: &[eth::TokenAddress]) -> HashMap<eth::TokenAddress, Metadata> {
        let to_fetch: Vec<_> = {
            let cache = self.cache.read().await;

            // Compute set of requested addresses that are not in cache.
            addresses
                .iter()
                // BUY_ETH_ADDRESS is just a marker and not a real address. We'll never be able to
                // fetch data for it so ignore it to avoid taking exclusive locks all the time.
                .filter(|address| !cache.contains_key(*address) && address.0.0 != BUY_ETH_ADDRESS)
                .cloned()
                .unique()
                .collect()
        };

        self.cache_missing_tokens(to_fetch).await;

        let cache = self.cache.read().await;
        // Return token infos from the cache.
        addresses
            .iter()
            .filter_map(|address| Some((*address, cache.get(address)?.clone())))
            .collect()
    }
}
