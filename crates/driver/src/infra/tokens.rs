use {
    crate::infra::{Ethereum, blockchain},
    anyhow::Result,
    eth_domain_types as eth,
    ethrpc::block_stream::{self, CurrentBlockWatcher},
    futures::{FutureExt, StreamExt},
    itertools::Itertools,
    model::order::BUY_ETH_ADDRESS,
    request_sharing::BoxRequestSharing,
    std::{
        collections::HashMap,
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
    /// If this flag is true a background task will continuously
    /// update this token's [`Metadata::balance`] field.
    pub monitor_balance: bool,
}

#[derive(Clone)]
pub struct Fetcher(Arc<Inner>);

impl Fetcher {
    pub fn new(eth: &Ethereum) -> Self {
        let eth = eth.with_metric_label("tokenInfos".into());
        let block_stream = eth.current_block().clone();
        let inner = Arc::new(Inner {
            eth,
            cache: RwLock::new(HashMap::new()),
            requests: BoxRequestSharing::labelled("token_info".into()),
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

    /// Tells the cache to continuously update the settlement contract balance
    /// of the given tokens if they are already present in the cache. That only
    /// really makes sense for tokens where internal buffer trading is allowed.
    pub fn keep_track_of_balances<'a>(&self, tokens: impl IntoIterator<Item = &'a eth::Address>) {
        // most of the time no updates are needed so we first take a read lock to
        // check if we even have to take a write lock for updating the tokens at all
        let tokens_to_update: Vec<_> = {
            let cache = self.0.cache.read().unwrap();
            tokens
                .into_iter()
                .filter(|token| {
                    cache
                        .get(&((**token).into()))
                        .is_some_and(|entry| !entry.monitor_balance)
                })
                .collect()
        };
        if !tokens_to_update.is_empty() {
            let mut cache = self.0.cache.write().unwrap();
            tokens_to_update.into_iter().for_each(|token| {
                if let Some(entry) = cache.get_mut(&((*token).into())) {
                    entry.monitor_balance = true;
                }
            })
        }
    }
}

/// Runs a single cache update cycle whenever a new block arrives until the
/// fetcher is dropped.
async fn update_task(blocks: CurrentBlockWatcher, inner: std::sync::Weak<Inner>) {
    let mut stream = block_stream::into_stream(blocks);
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
    let settlement = *inner.eth.contracts().settlement().address();
    let futures: Vec<_> = {
        let cache = inner.cache.read().unwrap();
        cache
            .iter()
            .filter(|(_token, info)| info.monitor_balance)
            .map(|(token, _info)| {
                let erc20 = inner.eth.erc20(*token);
                async move { (erc20.address(), erc20.balance(settlement).await) }
            })
            .collect()
    };

    tracing::debug!(
        tokens = futures.len(),
        "updating settlement contract balances"
    );

    // Don't hold on to the lock while fetching balances to allow concurrent
    // updates. This may lead to new entries arriving in the meantime, however
    // their balances should already be up-to-date.
    let balances = futures::future::join_all(futures).await;

    let mut failed_updates = vec![];
    {
        let mut cache = inner.cache.write().unwrap();
        for (token, balance_result) in balances {
            let Ok(balance) = balance_result else {
                failed_updates.push(token);
                continue;
            };

            if let Some(entry) = cache.get_mut(&token) {
                entry.balance = balance;
            }
        }
    }

    if !failed_updates.is_empty() {
        tracing::info!(tokens = ?failed_updates, "failed to update token balance");
    }

    Ok(())
}

/// Provides metadata of tokens.
struct Inner {
    eth: Ethereum,
    cache: RwLock<HashMap<eth::TokenAddress, Metadata>>,
    requests: BoxRequestSharing<eth::TokenAddress, Option<(eth::TokenAddress, Metadata)>>,
}

impl Inner {
    /// Fetches `Metadata` of the requested tokens from a node.
    async fn fetch_token_infos(
        &self,
        tokens: &[eth::TokenAddress],
    ) -> Vec<Option<(eth::TokenAddress, Metadata)>> {
        let settlement = *self.eth.contracts().settlement().address();
        let futures = tokens.iter().map(|token| {
            let build_request = |token: &eth::TokenAddress| {
                let token = self.eth.erc20(*token);
                async move {
                    // Use `try_join` because these calls get batched under the hood
                    // so if one of them fails the others will as well.
                    // Also this way we won't get incomplete data for a token.
                    let (decimals, symbol, balance) = futures::future::try_join3(
                        token.decimals(),
                        token.symbol(),
                        token.balance(settlement),
                    )
                    .await
                    .ok()?;

                    Some((
                        token.address(),
                        Metadata {
                            decimals,
                            symbol,
                            balance,
                            monitor_balance: false,
                        },
                    ))
                }
                .boxed()
            };

            self.requests.shared_or_else(*token, build_request)
        });
        futures::future::join_all(futures).await
    }

    /// Ensures that all the missing tokens are in the cache afterwards while
    /// taking into account that the function might be called multiple times
    /// for the same tokens.
    async fn cache_missing_tokens(&self, tokens: &[eth::TokenAddress]) {
        if tokens.is_empty() {
            return;
        }

        let fetched = self.fetch_token_infos(tokens).await;
        {
            let cache = self.cache.read().unwrap();
            if tokens.iter().all(|token| cache.contains_key(token)) {
                // Often multiple callers are racing to fetch the same Metadata.
                // If somebody else already cached the data we don't want to take an
                // exclusive lock for nothing.
                return;
            }
        }
        self.cache
            .write()
            .unwrap()
            .extend(fetched.into_iter().flatten());
    }

    async fn get(&self, addresses: &[eth::TokenAddress]) -> HashMap<eth::TokenAddress, Metadata> {
        let to_fetch: Vec<_> = {
            let cache = self.cache.read().unwrap();

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

        self.cache_missing_tokens(&to_fetch).await;

        let cache = self.cache.read().unwrap();
        // Return token infos from the cache.
        addresses
            .iter()
            .filter_map(|address| Some((*address, cache.get(address)?.clone())))
            .collect()
    }
}
