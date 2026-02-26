use {
    crate::account_balances::{BalanceFetching, Query, TransferSimulationError},
    alloy::primitives::U256,
    anyhow::Result,
    ethrpc::block_stream::{CurrentBlockWatcher, into_stream},
    futures::{FutureExt, StreamExt, stream::{self, BoxStream}},
    itertools::Itertools,
    std::{
        collections::HashMap, pin, sync::{Arc, Mutex}
    },
    tracing::{Instrument, instrument},
};

type BlockNumber = u64;

/// Balances get removed from the cache after this many blocks without being
/// requested.
const EVICTION_TIME: BlockNumber = 5;

#[derive(Default)]
struct BalanceCache {
    last_seen_block: BlockNumber,
    data: HashMap<Query, BalanceEntry>,
}

impl BalanceCache {
    /// Retrieves cached balance and updates the `requested_at` field.
    fn get_cached_balance(&mut self, query: &Query) -> Option<U256> {
        match self.data.get_mut(query) {
            Some(entry) => {
                entry.requested_at = self.last_seen_block;
                Some(entry.balance)
            }
            None => None,
        }
    }

    /// Only updates existing balances. This should always be used in the
    /// background task.
    fn update_balance(&mut self, query: &Query, balance: U256, update_block: BlockNumber) {
        if update_block < self.last_seen_block {
            // This should never realistically happen.
            return;
        }

        if let Some(entry) = self.data.get_mut(query) {
            entry.updated_at = update_block;
            entry.balance = balance;
        }
    }

    /// Only inserts new balances. This should always be used when we needed to
    /// fetch a balance because it was requested by a backend component.
    fn insert_balance(&mut self, query: Query, balance: U256, requested_at: BlockNumber) {
        self.data.insert(
            query,
            BalanceEntry {
                requested_at,
                updated_at: requested_at,
                balance,
            },
        );
    }
}

#[derive(Debug, Clone)]
struct BalanceEntry {
    requested_at: BlockNumber,
    updated_at: BlockNumber,
    balance: U256,
}

pub struct Balances {
    inner: Arc<dyn BalanceFetching>,
    balance_cache: Arc<Mutex<BalanceCache>>,
}

impl Balances {
    pub fn new(inner: Arc<dyn BalanceFetching>) -> Self {
        Self {
            inner,
            balance_cache: Default::default(),
        }
    }
}

struct CacheResponse<'a> {
    cached: Vec<(&'a Query, Result<U256>)>,
    missing: Vec<&'a Query>,
    requested_at: BlockNumber,
}

impl Balances {
    fn get_cached_balances<'a>(&'a self, queries: &'a [&'a Query]) -> CacheResponse<'a> {
        let mut cache = self.balance_cache.lock().unwrap();
        let (cached, missing) = queries
            .iter()
            .partition_map(|query| match cache.get_cached_balance(query) {
                Some(balance) => itertools::Either::Left((*query, Ok(balance))),
                None => itertools::Either::Right(*query),
            });
        CacheResponse {
            cached,
            missing,
            requested_at: cache.last_seen_block,
        }
    }

    /// Spawns task that refreshes the cached balances on every new block.
    pub fn spawn_background_task(&self, block_stream: CurrentBlockWatcher) {
        let inner = self.inner.clone();
        let cache = self.balance_cache.clone();
        let mut stream = into_stream(block_stream);

        let task = async move {
            while let Some(block) = stream.next().await {
                let balances_to_update = {
                    let mut cache = cache.lock().unwrap();
                    cache.last_seen_block = block.number;
                    cache
                        .data
                        .iter()
                        .filter_map(|(query, entry)| {
                            // Only update balances that have been requested recently.
                            let oldest_allowed_request =
                                cache.last_seen_block.saturating_sub(EVICTION_TIME);
                            (entry.requested_at >= oldest_allowed_request).then_some(query.clone())
                        })
                        .collect_vec()
                };

                let results: Vec<_> = inner.get_balances(&balances_to_update).collect().await;

                let mut cache = cache.lock().unwrap();
                balances_to_update
                    .into_iter()
                    .zip(results)
                    .for_each(|(query, result)| {
                        if let Ok(balance) = result {
                            cache.update_balance(&query, balance, block.number);
                        }
                    });
                cache.data.retain(|_, value| {
                    // Only keep balances where we know we have the most recent data.
                    value.updated_at >= block.number
                });
            }
            tracing::error!("block stream terminated unexpectedly");
        };
        tokio::spawn(task.instrument(tracing::info_span!("balance_cache")));
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    // #[instrument(skip_all)]
    fn get_balances<'a, 'b>(&'a self, queries: &'a [&'a Query]) -> BoxStream<'a, (&'a Query, Result<U256>)> {
        let CacheResponse {
            cached,
            missing,
            requested_at,
        } = self.get_cached_balances(queries);

        let cached_stream = stream::iter(cached);
        if missing.is_empty() {
            return cached_stream.boxed();
        }

        // complains about `missing` being borrowed from a local context
        let missing_stream = self.inner.get_balances(&missing);
        // todo: inspect `missing_stream` and update cache in chunks

        cached_stream.chain(missing_stream).boxed()
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        // This only gets called when creating or replacing an order which doesn't
        // profit from caching.
        self.inner.can_transfer(query, amount).await
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
            .withf(|arg| arg == [query(1)])
            .returning(|_| stream::iter([Ok(U256::ONE)]).boxed());

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        let result = fetcher.get_balances(&[query(1)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
        // Fetches balance from cache and skips calling `inner`.
        let result = fetcher.get_balances(&[query(1)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn does_not_cache_errors() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(2)
            .withf(|arg| arg == [query(1)])
            .returning(|_| stream::iter([Err(anyhow::anyhow!("some error"))]).boxed());

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        assert!(
            fetcher
                .get_balances(&[query(1)])
                .next()
                .await
                .unwrap()
                .is_err()
        );
        // 2nd call to `inner`.
        assert!(
            fetcher
                .get_balances(&[query(1)])
                .next()
                .await
                .unwrap()
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
            .withf(|arg| arg == [query(1)])
            .returning(|_| stream::iter([Ok(U256::ONE)]).boxed());

        let fetcher = Balances::new(Arc::new(inner));
        fetcher.spawn_background_task(receiver);

        // 1st call to `inner`. Balance gets cached.
        let result = fetcher.get_balances(&[query(1)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

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
        let result = fetcher.get_balances(&[query(1)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn can_return_new_and_cached_results_in_same_call() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(1)])
            .returning(|_| stream::iter([Ok(U256::ONE)]).boxed());
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(2)])
            .returning(|_| stream::iter([Ok(U256::from(2))]).boxed());

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner` putting balance 1 into the cache.
        let result = fetcher.get_balances(&[query(1)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        // Fetches balance 1 from cache and balance 2 fresh. (2nd call to `inner`)
        let result = fetcher
            .get_balances(&[query(1), query(2)])
            .collect::<Vec<_>>()
            .await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
        assert_eq!(result[1].as_ref().unwrap(), &U256::from(2));

        // Now balance 2 is also in the cache. Skipping call to `inner`.
        let result = fetcher.get_balances(&[query(2)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::from(2));
    }

    #[tokio::test]
    async fn unused_balances_get_evicted() {
        let first_block = BlockInfo::default();
        let (sender, receiver) = tokio::sync::watch::channel(first_block);

        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(7)
            .returning(|_| stream::iter([Ok(U256::ONE)]).boxed());

        let fetcher = Balances::new(Arc::new(inner));
        fetcher.spawn_background_task(receiver);

        let cached_entry = || {
            let cache = fetcher.balance_cache.lock().unwrap();
            cache.data.get(&query(1)).cloned()
        };

        assert!(cached_entry().is_none());
        // 1st call to `inner`. Balance gets cached.
        let result = fetcher.get_balances(&[query(1)]).collect::<Vec<_>>().await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

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
}
