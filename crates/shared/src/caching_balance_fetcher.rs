use crate::{
    account_balances::{BalanceFetching, Query, TransferSimulationError},
    current_block::{into_stream, CurrentBlockStream},
};
use anyhow::Result;
use futures::StreamExt;
use itertools::Itertools;
use model::order::SellTokenSource;
use primitive_types::{H160, U256};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tracing::Instrument;

#[derive(Default)]
struct BalanceCache {
    /// Block number the cached data was collected from.
    block: u64,
    data: HashMap<Query, U256>,
}

pub struct CachingBalanceFetcher {
    inner: Arc<dyn BalanceFetching>,
    balance_cache: Arc<RwLock<BalanceCache>>,
}

impl CachingBalanceFetcher {
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
    // On what block number the balances got collected.
    block: u64,
}

impl CachingBalanceFetcher {
    fn get_cached_balances(&self, queries: &[Query]) -> CacheResponse {
        let read_lock = self.balance_cache.read().unwrap();
        let block = read_lock.block;
        let (cached, missing) = queries.iter().enumerate().partition_map(|(i, query)| {
            let cached_value = read_lock.data.get(query);
            match cached_value {
                Some(balance) => itertools::Either::Left((i, Ok(*balance))),
                None => itertools::Either::Right(i),
            }
        });
        CacheResponse {
            cached,
            missing,
            block,
        }
    }

    /// Updates the cache if the it doesn't contain more recent data.
    fn update_balance_cache(
        cache: &RwLock<BalanceCache>,
        start_block: u64,
        updates: Vec<(Query, U256)>,
    ) {
        let mut write_lock = cache.write().unwrap();
        if write_lock.block > start_block {
            // Newer data might already be availble which we don't want to overwrite.
            return;
        }
        write_lock.data.extend(updates);
    }

    /// Spawns task that refreshes the cached balances on every new block.
    pub fn spawn_background_task(&self, block_stream: CurrentBlockStream) {
        let inner = self.inner.clone();
        let cache = self.balance_cache.clone();
        let mut stream = into_stream(block_stream);

        let task = async move {
            while let Some(block) = stream.next().await {
                // invalidate cache immediately
                let old_cache = {
                    let empty_cache = BalanceCache {
                        block: block.number,
                        data: Default::default(),
                    };
                    let mut old_cache = cache.write().unwrap();
                    std::mem::replace(&mut *old_cache, empty_cache)
                };

                let queries: Vec<_> = old_cache.data.into_keys().collect();
                let results = inner.get_balances(&queries).await;
                let updates = queries
                    .into_iter()
                    .zip(results.into_iter())
                    .filter_map(|(query, result)| Some((query, result.ok()?)))
                    .collect();
                Self::update_balance_cache(&cache, block.number, updates);
            }
            tracing::error!("block stream terminated unexpectedly");
        };
        tokio::spawn(task.instrument(tracing::info_span!("balance_cache")));
    }
}

#[async_trait::async_trait]
impl BalanceFetching for CachingBalanceFetcher {
    async fn get_balances(&self, queries: &[Query]) -> Vec<Result<U256>> {
        let CacheResponse {
            mut cached,
            missing,
            block: initial_block,
        } = self.get_cached_balances(queries);

        let missing_queries: Vec<Query> = missing.iter().map(|i| queries[*i]).collect();
        let new_balances = self.inner.get_balances(&missing_queries).await;

        let updates = missing
            .iter()
            .zip(new_balances.iter())
            .filter_map(|(i, result)| Some((queries[*i], *result.as_ref().ok()?)))
            .collect();
        Self::update_balance_cache(&self.balance_cache, initial_block, updates);

        cached.extend(missing.into_iter().zip(new_balances.into_iter()));
        cached.sort_by_key(|(i, _)| *i);
        cached.into_iter().map(|(_, balance)| balance).collect()
    }

    async fn can_transfer(
        &self,
        token: H160,
        from: H160,
        amount: U256,
        source: SellTokenSource,
    ) -> Result<(), TransferSimulationError> {
        // This only gets called when creating or replacing an order which doesn't profit from
        // caching.
        self.inner.can_transfer(token, from, amount, source).await
    }
}
