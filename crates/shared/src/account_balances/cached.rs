use {
    crate::account_balances::{BalanceFetching, Query, TransferSimulationError},
    alloy::primitives::U256,
    anyhow::Result,
    dashmap::DashMap,
    ethrpc::block_stream::{CurrentBlockWatcher, into_stream},
    futures::{
        FutureExt,
        StreamExt,
        future::BoxFuture,
        stream::{self, BoxStream},
    },
    itertools::Itertools,
    std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    tracing::{Instrument, instrument},
};

type BlockNumber = u64;

/// Balances get removed from the cache after this many blocks without being
/// requested.
const EVICTION_TIME: BlockNumber = 5;

#[derive(Debug, Clone)]
struct BalanceEntry {
    requested_at: BlockNumber,
    updated_at: BlockNumber,
    balance: U256,
}

pub struct Inner {
    fetcher: Arc<dyn BalanceFetching>,
    cache: DashMap<Query, BalanceEntry>,
    last_seen_block: AtomicU64,
}

pub struct Balances(Arc<Inner>);

impl Balances {
    pub fn new(inner: Arc<dyn BalanceFetching>) -> Self {
        Self(Arc::new(Inner {
            fetcher: inner,
            cache: Default::default(),
            last_seen_block: AtomicU64::new(0),
        }))
    }
}

struct CacheResponse {
    cached: Vec<(Query, Result<U256>)>,
    missing: Vec<Query>,
    requested_at: BlockNumber,
}

impl Balances {
    fn get_cached_balances(&self, queries: Vec<Query>) -> CacheResponse {
        let requested_at = self.0.last_seen_block.load(Ordering::Relaxed);
        let (cached, missing) =
            queries
                .into_iter()
                .partition_map(|query| match self.0.cache.get_mut(&query) {
                    Some(mut entry) => {
                        entry.requested_at = requested_at;
                        itertools::Either::Left((query, Ok(entry.balance)))
                    }
                    None => itertools::Either::Right(query),
                });
        CacheResponse {
            cached,
            missing,
            requested_at,
        }
    }

    /// Spawns task that refreshes the cached balances on every new block.
    pub fn spawn_background_task(&self, block_stream: CurrentBlockWatcher) {
        let inner = self.0.clone();
        let mut stream = into_stream(block_stream);

        let task = async move {
            while let Some(block) = stream.next().await {
                inner.last_seen_block.store(block.number, Ordering::Relaxed);

                let oldest_allowed_request = block.number.saturating_sub(EVICTION_TIME);
                let balances_to_update: Vec<_> = inner
                    .cache
                    .iter()
                    .filter_map(|entry| {
                        (entry.requested_at >= oldest_allowed_request).then(|| entry.key().clone())
                    })
                    .collect();

                inner
                    .fetcher
                    .get_balances(balances_to_update)
                    .for_each_concurrent(100, |fut| {
                        let inner = inner.clone();
                        async move {
                            let (query, result) = fut.await;
                            if let Ok(balance) = result
                                && let Some(mut entry) = inner.cache.get_mut(&query)
                                && block.number >= entry.updated_at
                            {
                                entry.updated_at = block.number;
                                entry.balance = balance;
                            }
                        }
                    })
                    .await;

                // this could already be done when we fetch the items to clone...
                inner
                    .cache
                    .retain(|_, entry| entry.updated_at >= block.number);
            }
            tracing::error!("block stream terminated unexpectedly");
        };
        tokio::spawn(task.instrument(tracing::info_span!("balance_cache")));
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    fn get_balances(
        &self,
        queries: Vec<Query>,
    ) -> BoxStream<'_, BoxFuture<'static, (Query, anyhow::Result<U256>)>> {
        let CacheResponse {
            cached,
            missing,
            requested_at,
        } = self.get_cached_balances(queries);

        let cached_stream = stream::iter(cached.into_iter().map(|res| async move { res }.boxed()));
        if missing.is_empty() {
            return cached_stream.boxed();
        }

        let inner = self.0.clone();
        let missing_stream = self.0.fetcher.get_balances(missing).map(move |fut| {
            let inner = inner.clone();
            async move {
                let (query, result) = fut.await;
                if let Ok(balance) = &result {
                    inner.cache.insert(
                        query.clone(),
                        BalanceEntry {
                            requested_at,
                            updated_at: requested_at,
                            balance: *balance,
                        },
                    );
                }
                (query, result)
            }
            .boxed()
        });

        cached_stream.chain(missing_stream).boxed()
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        // This only gets called when creating or replacing an order which doesn't
        // profit from caching.
        self.0.fetcher.can_transfer(query, amount).await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::account_balances::MockBalanceFetching,
        alloy::primitives::Address,
        ethrpc::block_stream::BlockInfo,
        futures::stream,
        model::order::SellTokenSource,
    };

    fn query(token: u8) -> Query {
        Query {
            owner: Address::repeat_byte(1),
            token: Address::repeat_byte(token),
            source: SellTokenSource::Erc20,
            interactions: vec![],
            balance_override: None,
        }
    }

    #[tokio::test]
    async fn caches_ok_results() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| *arg == [query(1)])
            .returning(|queries| with_balance(queries, || Ok(U256::ONE)));

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        let result = fetcher
            .get_balances(vec![query(1)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);
        // Fetches balance from cache and skips calling `inner`.
        let result = fetcher
            .get_balances(vec![query(1)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn does_not_cache_errors() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(2)
            .withf(|arg| *arg == [query(1)])
            .returning(|queries| with_balance(queries, || Err(anyhow::anyhow!("some error"))));

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        assert!(
            fetcher
                .get_balances(vec![query(1)])
                .next()
                .await
                .unwrap()
                .await
                .1
                .is_err()
        );
        // 2nd call to `inner`.
        assert!(
            fetcher
                .get_balances(vec![query(1)])
                .next()
                .await
                .unwrap()
                .await
                .1
                .is_err()
        );
    }

    #[tokio::test]
    async fn background_task_updates_cache_on_new_block() {
        let first_block = BlockInfo::default();
        let (sender, receiver) = tokio::sync::watch::channel(first_block);

        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(2)
            .withf(|arg| *arg == [query(1)])
            .returning(|queries| with_balance(queries, || Ok(U256::ONE)));

        let fetcher = Balances::new(Arc::new(inner));
        fetcher.spawn_background_task(receiver);

        // 1st call to `inner`. Balance gets cached.
        let result = fetcher
            .get_balances(vec![query(1)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);

        // New block gets detected.
        sender
            .send(BlockInfo {
                number: 1,
                ..Default::default()
            })
            .unwrap();
        // Wait for block to be noticed and cache to be updated. (2nd call to inner)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Balance was already updated so this will hit the cache and skip calling
        // `inner`.
        let result = fetcher
            .get_balances(vec![query(1)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn can_return_new_and_cached_results_in_same_call() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| *arg == [query(1)])
            .returning(|queries| with_balance(queries, || Ok(U256::ONE)));
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| *arg == [query(2)])
            .returning(|queries| with_balance(queries, || Ok(U256::from(2))));

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner` putting balance 1 into the cache.
        let result = fetcher
            .get_balances(vec![query(1)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);

        // Fetches balance 1 from cache and balance 2 fresh. (2nd call to `inner`)
        let result = fetcher
            .get_balances(vec![query(1), query(2)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);
        assert_eq!(result[1].1.as_ref().unwrap(), &U256::from(2));

        // Now balance 2 is also in the cache. Skipping call to `inner`.
        let result = fetcher
            .get_balances(vec![query(2)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::from(2));
    }

    #[tokio::test]
    async fn unused_balances_get_evicted() {
        let first_block = BlockInfo::default();
        let (sender, receiver) = tokio::sync::watch::channel(first_block);

        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(7)
            .returning(|queries| with_balance(queries, || Ok(U256::ONE)));

        let fetcher = Balances::new(Arc::new(inner));
        fetcher.spawn_background_task(receiver);

        let cached_entry = || fetcher.0.cache.get(&query(1)).map(|e| e.clone());

        assert!(cached_entry().is_none());
        // 1st call to `inner`. Balance gets cached.
        let result = fetcher
            .get_balances(vec![query(1)])
            .buffered(10)
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].1.as_ref().unwrap(), &U256::ONE);

        for block in 1..=EVICTION_TIME + 1 {
            assert!(cached_entry().is_some());
            // New block gets detected.
            sender
                .send(BlockInfo {
                    number: block,
                    ..Default::default()
                })
                .unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        assert!(cached_entry().is_none());
    }

    fn with_balance(
        queries: Vec<Query>,
        value: impl Fn() -> Result<U256> + Send + Sync + 'static,
    ) -> BoxStream<'static, BoxFuture<'static, (Query, Result<U256>)>> {
        let value = Arc::new(value);
        let results: Vec<_> = queries
            .into_iter()
            .map(|q| {
                let value = value.clone();
                async move { (q, value()) }.boxed()
            })
            .collect();
        stream::iter(results).boxed()
    }
}
