use {
    crate::{BalanceFetching, Query, TransferSimulationError},
    alloy_primitives::{Address, U256},
    anyhow::Result,
    ethrpc::block_stream::{CurrentBlockWatcher, into_stream},
    futures::StreamExt,
    itertools::Itertools,
    model::order::SellTokenSource,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    tracing::instrument,
};

#[derive(Default)]
struct BalanceCache {
    data: HashMap<Query, BalanceEntry>,
}

impl BalanceCache {
    /// Retrieves cached balance and updates the `requested_at` field.
    fn get_cached_balance(&mut self, query: &Query, now: Instant) -> Option<U256> {
        match self.data.get_mut(query) {
            Some(entry) => {
                entry.requested_at = now;
                Some(entry.balance)
            }
            None => None,
        }
    }

    /// Only updates existing balances. This should always be used in the
    /// background task. Has no write-ordering guard, so it relies on refreshes
    /// staying serialized (see [`Balances::spawn_background_task`]).
    fn update_balance(&mut self, query: &Query, balance: U256) {
        if let Some(entry) = self.data.get_mut(query) {
            entry.balance = balance;
        }
    }

    fn remove(&mut self, query: &Query) {
        self.data.remove(query);
    }

    /// Only inserts new balances. This should always be used when we needed to
    /// fetch a balance because it was requested by a backend component.
    fn insert_balance(&mut self, query: Query, balance: U256, now: Instant) {
        self.data.insert(
            query,
            BalanceEntry {
                requested_at: now,
                balance,
            },
        );
    }
}

#[derive(Debug, Clone)]
struct BalanceEntry {
    requested_at: Instant,
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

struct CacheResponse {
    // The indices and results of queries that were in the cache.
    cached: Vec<(usize, Result<U256>)>,
    // Indices of queries that were not in the cache.
    missing: Vec<usize>,
}

impl Balances {
    fn get_cached_balances(&self, queries: &[Query]) -> CacheResponse {
        let mut cache = self.balance_cache.lock().unwrap();
        let now = Instant::now();
        let (cached, missing) = queries
            .iter()
            .enumerate()
            .partition_map(|(i, query)| match cache.get_cached_balance(query, now) {
                Some(balance) => itertools::Either::Left((i, Ok(balance))),
                None => itertools::Either::Right(i),
            });
        CacheResponse { cached, missing }
    }

    /// Spawns task that refreshes the cached balances on every new block.
    /// Refreshes must stay serialized: a second concurrent refresh could
    /// overwrite a newer balance with the result of an older, slower fetch.
    pub fn spawn_background_task(
        &self,
        block_stream: CurrentBlockWatcher,
        eviction_time: Duration,
        refresh_cooldown: Duration,
    ) {
        let inner = self.inner.clone();
        let cache = self.balance_cache.clone();
        let mut stream = into_stream(block_stream);

        let task = async move {
            while stream.next().await.is_some() {
                Self::refresh_balances(inner.as_ref(), &cache, eviction_time).await;
                // Blocks arriving during the cooldown collapse into a single
                // refresh, so a fast chain does not refresh the whole cache
                // once per block.
                tokio::time::sleep(refresh_cooldown).await;
            }
            tracing::error!("block stream terminated unexpectedly");
        };
        tokio::spawn(task);
    }

    /// Evicts entries that haven't been requested within `eviction_time` and
    /// refreshes the remaining cached balances.
    #[instrument(skip_all)]
    async fn refresh_balances(
        fetcher: &dyn BalanceFetching,
        cache: &Mutex<BalanceCache>,
        eviction_time: Duration,
    ) {
        let balances_to_update = {
            let mut cache = cache.lock().unwrap();
            let now = Instant::now();
            let mut to_update = Vec::with_capacity(cache.data.len());
            cache.data.retain(|query, entry| {
                let recently_requested =
                    now.saturating_duration_since(entry.requested_at) <= eviction_time;
                if recently_requested {
                    to_update.push(query.clone());
                }
                recently_requested
            });
            to_update
        };

        let results = fetcher.get_balances(&balances_to_update).await;

        let mut cache = cache.lock().unwrap();
        balances_to_update
            .into_iter()
            .zip(results)
            .for_each(|(query, result)| match result {
                Ok(balance) => cache.update_balance(&query, balance),
                // Drop the entry so we don't keep serving a balance we no
                // longer know to be current.
                Err(_) => cache.remove(&query),
            });
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        let CacheResponse {
            mut cached,
            missing,
        } = self.get_cached_balances(queries);

        if missing.is_empty() {
            return cached.into_iter().map(|(_, result)| result).collect();
        }

        let missing_queries: Vec<Query> = missing.iter().map(|i| queries[*i].clone()).collect();
        let new_balances = self.inner.get_balances(&missing_queries).await;

        {
            let mut cache = self.balance_cache.lock().unwrap();
            let now = Instant::now();
            for (query, result) in missing_queries.into_iter().zip(new_balances.iter()) {
                if let Ok(balance) = result {
                    cache.insert_balance(query, *balance, now)
                }
            }
        }

        cached.extend(missing.into_iter().zip(new_balances));
        cached.sort_by_key(|(i, _)| *i);
        cached.into_iter().map(|(_, balance)| balance).collect()
    }

    async fn can_transfer(
        &self,
        query: &Query,
        amount: U256,
    ) -> Result<(), TransferSimulationError> {
        // This only gets called when creating or replacing an order which
        // doesn't profit from caching.
        self.inner.can_transfer(query, amount).await
    }

    async fn allowance(
        &self,
        owner: Address,
        token: Address,
        source: SellTokenSource,
    ) -> Result<U256> {
        // This only gets called when creating or replacing an order which
        // doesn't profit from caching.
        self.inner.allowance(owner, token, source).await
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::MockBalanceFetching,
        alloy_primitives::Address,
        ethrpc::block_stream::BlockInfo,
        model::order::SellTokenSource,
    };

    const TEST_EVICTION_TIME: Duration = Duration::from_millis(200);
    const TEST_REFRESH_COOLDOWN: Duration = Duration::from_millis(1);

    fn query(token: u8) -> Query {
        Query {
            owner: Address::repeat_byte(1),
            token: Address::repeat_byte(token),
            source: SellTokenSource::Erc20,
            interactions: vec![],
            balance_override: None,
        }
    }

    /// Reads an entry without marking it as requested, so polling for a
    /// background update does not keep the entry alive.
    fn cached_balance(fetcher: &Balances, query: &Query) -> Option<U256> {
        let cache = fetcher.balance_cache.lock().unwrap();
        cache.data.get(query).map(|entry| entry.balance)
    }

    /// Polls until the background task produced the expected state. Returns as
    /// soon as it did, so a generous timeout costs nothing in the happy case.
    async fn wait_for(what: &str, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    #[tokio::test]
    async fn caches_ok_results() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(1)])
            .returning(|_| vec![Ok(U256::ONE)]);

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
        // Fetches balance from cache and skips calling `inner`.
        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn does_not_cache_errors() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(2)
            .withf(|arg| arg == [query(1)])
            .returning(|_| vec![Err(anyhow::anyhow!("some error"))]);

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        assert!(fetcher.get_balances(&[query(1)]).await[0].is_err());
        // 2nd call to `inner`.
        assert!(fetcher.get_balances(&[query(1)]).await[0].is_err());
    }

    #[tokio::test]
    async fn background_task_updates_cache_on_new_block() {
        let (sender, receiver) = tokio::sync::watch::channel(BlockInfo::default());

        let mut inner = MockBalanceFetching::new();
        // 1st call to `inner`. Balance gets cached.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(1)])
            .returning(|_| vec![Ok(U256::ONE)]);
        // 2nd call to `inner`. Background refresh sees a new balance.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(1)])
            .returning(|_| vec![Ok(U256::from(2))]);

        let fetcher = Balances::new(Arc::new(inner));

        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        // The task only sees the latest block, so sending before it first gets
        // polled keeps this to a single refresh.
        fetcher.spawn_background_task(receiver, TEST_EVICTION_TIME, TEST_REFRESH_COOLDOWN);
        sender
            .send(BlockInfo {
                number: 1,
                ..Default::default()
            })
            .unwrap();
        wait_for("refreshed balance", || {
            cached_balance(&fetcher, &query(1)) == Some(U256::from(2))
        })
        .await;

        // Balance was already updated so this will hit the cache and skip
        // calling `inner`.
        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::from(2));
    }

    #[tokio::test]
    async fn can_return_new_and_cached_results_in_same_call() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(1)])
            .returning(|_| vec![Ok(U256::ONE)]);
        inner
            .expect_get_balances()
            .times(1)
            .withf(|arg| arg == [query(2)])
            .returning(|_| vec![Ok(U256::from(2))]);

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner` putting balance 1 into the cache.
        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        // Fetches balance 1 from cache and balance 2 fresh. (2nd call to
        // `inner`)
        let result = fetcher.get_balances(&[query(1), query(2)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
        assert_eq!(result[1].as_ref().unwrap(), &U256::from(2));

        // Now balance 2 is also in the cache. Skipping call to `inner`.
        let result = fetcher.get_balances(&[query(2)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::from(2));
    }

    #[tokio::test]
    async fn failed_refresh_evicts_balance() {
        let (sender, receiver) = tokio::sync::watch::channel(BlockInfo::default());

        let mut inner = MockBalanceFetching::new();
        // 1st call to `inner`. Balance gets cached.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries| queries == [query(1)])
            .returning(|_| vec![Ok(U256::ONE)]);
        // 2nd call to `inner`. Background refresh fails.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries| queries == [query(1)])
            .returning(|_| vec![Err(anyhow::anyhow!("node error"))]);
        // 3rd call to `inner`. Balance is no longer cached.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries| queries == [query(1)])
            .returning(|_| vec![Ok(U256::from(2))]);

        let fetcher = Balances::new(Arc::new(inner));

        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        fetcher.spawn_background_task(receiver, TEST_EVICTION_TIME, TEST_REFRESH_COOLDOWN);
        sender
            .send(BlockInfo {
                number: 1,
                ..Default::default()
            })
            .unwrap();
        wait_for("failed refresh to drop the entry", || {
            cached_balance(&fetcher, &query(1)).is_none()
        })
        .await;

        // The failed refresh dropped the entry, so the next request has to go
        // to `inner` again and sees the new balance.
        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::from(2));
    }

    #[tokio::test]
    async fn unused_balances_get_evicted() {
        let (sender, receiver) = tokio::sync::watch::channel(BlockInfo::default());

        let mut inner = MockBalanceFetching::new();
        // 1st call to `inner`. Balance gets cached.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries| queries == [query(1)])
            .returning(|_| vec![Ok(U256::ONE)]);
        // 2nd call to `inner`. Entry is still within the eviction window, so it
        // gets refreshed rather than dropped.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries| queries == [query(1)])
            .returning(|_| vec![Ok(U256::from(2))]);
        // 3rd call to `inner`. Entry got evicted while building the list.
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries| queries.is_empty())
            .returning(|_| vec![]);

        let fetcher = Balances::new(Arc::new(inner));

        assert!(cached_balance(&fetcher, &query(1)).is_none());
        let result = fetcher.get_balances(&[query(1)]).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        fetcher.spawn_background_task(receiver, TEST_EVICTION_TIME, TEST_REFRESH_COOLDOWN);
        sender
            .send(BlockInfo {
                number: 1,
                ..Default::default()
            })
            .unwrap();
        wait_for("refresh within the eviction window", || {
            cached_balance(&fetcher, &query(1)) == Some(U256::from(2))
        })
        .await;

        // Let the entry go stale. Refreshing does not count as a request, so
        // this is the only place the test has to wait for real time.
        tokio::time::sleep(TEST_EVICTION_TIME).await;

        sender
            .send(BlockInfo {
                number: 2,
                ..Default::default()
            })
            .unwrap();
        wait_for("stale entry to be evicted", || {
            cached_balance(&fetcher, &query(1)).is_none()
        })
        .await;
    }
}
