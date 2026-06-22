use {
    crate::{BalanceFetching, BlockNumber, Query, TransferSimulationError},
    alloy_primitives::U256,
    anyhow::Result,
    ethrpc::block_stream::{CurrentBlockWatcher, into_stream},
    futures::StreamExt,
    itertools::Itertools,
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    },
    tracing::{Instrument, instrument},
};

#[derive(Debug, Clone, Copy)]
pub struct CachePolicy {
    staleness_tolerance: BlockNumber,
    eviction_time: BlockNumber,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            staleness_tolerance: Self::DEFAULT_STALENESS_TOLERANCE,
            eviction_time: Self::DEFAULT_EVICTION_TIME,
        }
    }
}

impl CachePolicy {
    const DEFAULT_EVICTION_TIME: BlockNumber = 5;
    const DEFAULT_STALENESS_TOLERANCE: BlockNumber = 1;

    pub fn new(staleness_tolerance: BlockNumber, eviction_time: BlockNumber) -> Self {
        Self {
            staleness_tolerance,
            eviction_time,
        }
    }

    pub fn with_staleness_tolerance(mut self, tolerance: BlockNumber) -> Self {
        self.staleness_tolerance = tolerance;
        self
    }

    pub fn with_eviction_time(mut self, time: BlockNumber) -> Self {
        self.eviction_time = time;
        self
    }

    pub fn staleness_tolerance(&self) -> BlockNumber {
        self.staleness_tolerance
    }

    pub fn eviction_time(&self) -> BlockNumber {
        self.eviction_time
    }

    fn is_within_staleness_tolerance(
        &self,
        updated_block: BlockNumber,
        current_block: BlockNumber,
    ) -> bool {
        let oldest_acceptable = current_block.saturating_sub(self.staleness_tolerance);
        updated_block >= oldest_acceptable && updated_block <= current_block
    }

    fn should_retain(&self, block: BlockNumber, current_block: BlockNumber) -> bool {
        block >= current_block.saturating_sub(self.eviction_time)
    }

    fn is_block_too_old_to_cache(&self, block: BlockNumber, current_block: BlockNumber) -> bool {
        !self.should_retain(block, current_block)
    }

    fn is_valid_block_stamp(&self, block: BlockNumber, current_block: BlockNumber) -> bool {
        if block > current_block {
            return false;
        }
        self.should_retain(block, current_block)
    }
}

#[derive(Debug, Clone)]
struct BalanceEntry {
    last_accessed_block: BlockNumber,
    updated_block: BlockNumber,
    balance: U256,
}

impl BalanceEntry {
    fn new(balance: U256, stamp: BlockNumber) -> Self {
        Self {
            last_accessed_block: stamp,
            updated_block: stamp,
            balance,
        }
    }

    fn update_last_accessed(&mut self, stamp: BlockNumber) {
        self.last_accessed_block = self.last_accessed_block.max(stamp);
    }

    fn is_within_staleness_tolerance(
        &self,
        current_block: BlockNumber,
        policy: &CachePolicy,
    ) -> bool {
        policy.is_within_staleness_tolerance(self.updated_block, current_block)
    }

    fn should_retain(&self, current_block: BlockNumber, policy: &CachePolicy) -> bool {
        policy.should_retain(self.last_accessed_block, current_block)
    }

    fn merge_update(&mut self, balance: U256, stamp: BlockNumber) {
        if stamp > self.updated_block {
            self.updated_block = stamp;
            self.balance = balance;
            self.update_last_accessed(stamp);
        }
    }
}

type SharedResult = Result<U256, Arc<anyhow::Error>>;

type InFlightKey = (Query, Option<BlockNumber>);

struct InFlightEntry {
    sender: tokio::sync::watch::Sender<Option<SharedResult>>,
}

impl InFlightEntry {
    fn new() -> Self {
        let (sender, _receiver) = tokio::sync::watch::channel(None);
        Self { sender }
    }

    fn subscribe(&self) -> tokio::sync::watch::Receiver<Option<SharedResult>> {
        self.sender.subscribe()
    }

    fn publish(&self, result: SharedResult) {
        let _ = self.sender.send(Some(result));
    }
}

type InFlightResult = tokio::sync::watch::Receiver<Option<SharedResult>>;
type InFlightFetchList = Vec<(usize, Query)>;
type InFlightWaitList = Vec<(usize, Query, InFlightResult)>;
type InFlightPartition = (InFlightFetchList, InFlightWaitList);

struct InFlightGuard {
    cache: Arc<Mutex<BalanceCache>>,
    keys: Vec<InFlightKey>,
}

impl InFlightGuard {
    fn new(cache: Arc<Mutex<BalanceCache>>, keys: Vec<InFlightKey>) -> Self {
        Self { cache, keys }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.complete_in_flight(&self.keys);
        }
    }
}

struct BalanceCache {
    current_block: BlockNumber,
    data: HashMap<Query, BalanceEntry>,
    policy: CachePolicy,
    in_flight: HashMap<InFlightKey, InFlightEntry>,
}

impl BalanceCache {
    fn new(policy: CachePolicy) -> Self {
        Self {
            current_block: 0,
            data: HashMap::new(),
            policy,
            in_flight: HashMap::new(),
        }
    }

    fn log_stale_block_warning(&self, incoming_block: BlockNumber, context: &str) {
        tracing::debug!(
            incoming_block = incoming_block,
            current_cached_block = self.current_block,
            "{}",
            context
        );
    }

    fn read_and_touch(&mut self, query: &Query) -> Option<U256> {
        match self.data.get_mut(query) {
            Some(entry) => {
                if entry.is_within_staleness_tolerance(self.current_block, &self.policy) {
                    entry.update_last_accessed(self.current_block);
                    Some(entry.balance)
                } else {
                    None
                }
            }
            None => None,
        }
    }

    fn update_balance(&mut self, query: &Query, balance: U256, update_block: BlockNumber) {
        if update_block < self.current_block {
            self.log_stale_block_warning(
                update_block,
                "update_balance: discarding stale block result",
            );
            return;
        }

        if let Some(entry) = self.data.get_mut(query)
            && update_block > entry.updated_block
        {
            entry.updated_block = update_block;
            entry.balance = balance;
        }
    }

    fn is_block_too_old_to_cache(&self, block: BlockNumber) -> bool {
        self.policy
            .is_block_too_old_to_cache(block, self.current_block)
    }

    fn is_valid_block_stamp(&self, block: BlockNumber) -> bool {
        self.policy.is_valid_block_stamp(block, self.current_block)
    }

    fn merge_results(&mut self, results: &[Result<U256>], queries: &[Query], stamp: BlockNumber) {
        for (query, result) in queries.iter().zip(results.iter()) {
            if let Ok(balance) = result {
                self.upsert_balance(query, *balance, stamp);
            }
        }
    }

    fn upsert_balance(&mut self, query: &Query, balance: U256, stamp: BlockNumber) {
        if !self.is_valid_block_stamp(stamp) {
            if stamp > self.current_block {
                tracing::warn!(
                    stamp = stamp,
                    current_block = self.current_block,
                    "upsert_balance: rejecting future block stamp (potential cache poisoning)"
                );
            } else {
                self.log_stale_block_warning(
                    stamp,
                    "upsert_balance: discarding too-old block result",
                );
            }
            return;
        }

        self.data
            .entry(query.clone())
            .and_modify(|entry| entry.merge_update(balance, stamp))
            .or_insert_with(|| BalanceEntry::new(balance, stamp));
    }

    fn cleanup_stale_entries(&mut self) {
        self.data
            .retain(|_, entry| entry.should_retain(self.current_block, &self.policy));
    }

    fn mark_in_flight(
        &mut self,
        query: &Query,
        block: Option<BlockNumber>,
    ) -> Option<InFlightResult> {
        let key = (query.clone(), block);
        if let Some(entry) = self.in_flight.get(&key) {
            Some(entry.subscribe())
        } else {
            self.in_flight.insert(key, InFlightEntry::new());
            None
        }
    }

    fn complete_in_flight(&mut self, keys: &[InFlightKey]) {
        for key in keys {
            self.in_flight.remove(key);
        }
    }

    fn store_in_flight_result(
        &mut self,
        query: &Query,
        block: Option<BlockNumber>,
        result: &Result<U256>,
    ) {
        let key = (query.clone(), block);
        if let Some(entry) = self.in_flight.get(&key) {
            let shared_result = match result {
                Ok(balance) => Ok(*balance),
                Err(err) => Err(Arc::new(anyhow::anyhow!("{:#}", err))),
            };
            entry.publish(shared_result);
        }
    }
}

pub struct Balances {
    inner: Arc<dyn BalanceFetching>,
    balance_cache: Arc<Mutex<BalanceCache>>,
}

impl Balances {
    pub fn new(inner: Arc<dyn BalanceFetching>) -> Self {
        Self::with_policy(inner, CachePolicy::default())
    }

    pub fn with_policy(inner: Arc<dyn BalanceFetching>, policy: CachePolicy) -> Self {
        Self {
            inner,
            balance_cache: Arc::new(Mutex::new(BalanceCache::new(policy))),
        }
    }

    fn with_cache_mut<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut BalanceCache) -> R,
    {
        self.balance_cache
            .lock()
            .map(|mut cache| f(&mut cache))
            .map_err(|err| {
                tracing::error!("cache mutex poisoned: {}", err);
            })
            .ok()
    }

    fn with_cache<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&BalanceCache) -> R,
    {
        self.balance_cache
            .lock()
            .map(|cache| f(&cache))
            .map_err(|err| {
                tracing::error!("cache mutex poisoned: {}", err);
            })
            .ok()
    }
}

struct CacheResponse {
    cached: Vec<(usize, Result<U256>)>,
    missing: Vec<usize>,
}

impl Balances {
    fn get_cached_balances(&self, queries: &[Query]) -> CacheResponse {
        self.with_cache_mut(|cache| {
            let (cached, missing) = queries.iter().enumerate().partition_map(|(i, query)| {
                match cache.read_and_touch(query) {
                    Some(balance) => itertools::Either::Left((i, Ok(balance))),
                    None => itertools::Either::Right(i),
                }
            });
            CacheResponse { cached, missing }
        })
        .unwrap_or_else(|| CacheResponse {
            cached: vec![],
            missing: queries.iter().enumerate().map(|(i, _)| i).collect(),
        })
    }

    fn apply_fetch_results(&self, results: &[Result<U256>], queries: &[Query], stamp: BlockNumber) {
        let has_any_ok = results.iter().any(|r| r.is_ok());
        if !has_any_ok {
            tracing::trace!("skipping cache update: all fetch results failed");
            return;
        }

        self.with_cache_mut(|cache| {
            if cache.is_block_too_old_to_cache(stamp) {
                tracing::debug!(
                    requested_block = stamp,
                    current_block = cache.current_block,
                    "discarding stale results"
                );
                return;
            }

            cache.merge_results(results, queries, stamp);
        });
    }

    async fn handle_block_pinned_request(
        &self,
        queries: &[Query],
        bn: BlockNumber,
    ) -> Vec<Result<U256>> {
        if queries.is_empty() {
            return vec![];
        }

        let (needs_fetch_queries, needs_wait_list) = self
            .with_cache_mut(|cache| {
                let (needs_fetch, needs_wait): (Vec<_>, Vec<_>) =
                    queries.iter().enumerate().partition_map(|(i, query)| {
                        if let Some(receiver) = cache.mark_in_flight(query, Some(bn)) {
                            itertools::Either::Right((i, query.clone(), receiver))
                        } else {
                            itertools::Either::Left((i, query.clone()))
                        }
                    });
                (needs_fetch, needs_wait)
            })
            .unwrap_or_else(|| {
                (
                    queries
                        .iter()
                        .enumerate()
                        .map(|(i, q)| (i, q.clone()))
                        .collect(),
                    vec![],
                )
            });

        if needs_fetch_queries.is_empty() {
            tracing::trace!(
                block = bn,
                query_count = queries.len(),
                "block-pinned request: all queries in-flight, waiting"
            );
            let waited_results = self.wait_for_in_flight(needs_wait_list).await;
            return self.reconstruct_results(queries.len(), vec![], waited_results);
        }

        tracing::debug!(
            block = bn,
            query_count = queries.len(),
            fetch_count = needs_fetch_queries.len(),
            wait_count = needs_wait_list.len(),
            "block-pinned request: fetching and waiting (no caching)"
        );

        let (fetched_results, waited_results) = tokio::join!(
            self.fetch_in_flight_for_block(needs_fetch_queries, bn),
            self.wait_for_in_flight(needs_wait_list)
        );

        self.reconstruct_results(queries.len(), fetched_results, waited_results)
    }

    fn reconstruct_results(
        &self,
        total_len: usize,
        fetched_results: Vec<((usize, Query), Result<U256>)>,
        waited_results: Vec<(usize, Result<U256>)>,
    ) -> Vec<Result<U256>> {
        let mut results: Vec<Option<Result<U256>>> = (0..total_len).map(|_| None).collect();

        for ((i, _), result) in fetched_results {
            results[i] = Some(result);
        }

        for (i, result) in waited_results {
            results[i] = Some(result);
        }

        results
            .into_iter()
            .map(|r| r.expect("all indices must be filled"))
            .collect()
    }

    async fn handle_none_request(&self, queries: &[Query]) -> Vec<Result<U256>> {
        let CacheResponse { cached, missing } = self.get_cached_balances(queries);

        if missing.is_empty() {
            return cached.into_iter().map(|(_, result)| result).collect();
        }

        let missing_queries: Vec<Query> = missing.iter().map(|i| queries[*i].clone()).collect();

        let partition_result = self.partition_in_flight(&missing_queries, None);
        let (needs_fetch, needs_wait) = match partition_result {
            Ok(partition) => partition,
            Err(_) => {
                return self
                    .fetch_and_merge(&missing_queries, &missing, cached, queries.len())
                    .await;
            }
        };

        let (fetched_results, waited_results) = tokio::join!(
            self.fetch_in_flight(needs_fetch),
            self.wait_for_in_flight(needs_wait)
        );

        self.merge_in_flight_results(
            queries.len(),
            cached,
            &missing,
            fetched_results,
            waited_results,
        )
    }

    fn partition_in_flight(
        &self,
        missing_queries: &[Query],
        block: Option<BlockNumber>,
    ) -> Result<InFlightPartition, ()> {
        match self.balance_cache.lock() {
            Ok(mut cache) => Ok(missing_queries
                .iter()
                .enumerate()
                .partition_map(|(i, query)| {
                    if let Some(cell) = cache.mark_in_flight(query, block) {
                        itertools::Either::Right((i, query.clone(), cell))
                    } else {
                        itertools::Either::Left((i, query.clone()))
                    }
                })),
            Err(_) => Err(()),
        }
    }

    async fn fetch_in_flight(
        &self,
        needs_fetch: InFlightFetchList,
    ) -> Vec<((usize, Query), Result<U256>)> {
        if needs_fetch.is_empty() {
            return vec![];
        }

        let fetch_queries: Vec<Query> = needs_fetch.iter().map(|(_, q)| q.clone()).collect();
        let fetch_block = self.with_cache(|cache| cache.current_block).unwrap_or(0);

        let keys: Vec<InFlightKey> = fetch_queries.iter().map(|q| (q.clone(), None)).collect();
        let _guard = InFlightGuard::new(self.balance_cache.clone(), keys);

        let results = self
            .inner
            .get_balances(&fetch_queries, Some(fetch_block))
            .await;

        debug_assert_eq!(
            results.len(),
            fetch_queries.len(),
            "get_balances contract violation"
        );

        self.publish_in_flight_results(&fetch_queries, &results, None);
        self.apply_fetch_results(&results, &fetch_queries, fetch_block);

        needs_fetch.into_iter().zip(results).collect()
    }

    async fn fetch_in_flight_for_block(
        &self,
        needs_fetch: InFlightFetchList,
        block: BlockNumber,
    ) -> Vec<((usize, Query), Result<U256>)> {
        if needs_fetch.is_empty() {
            return vec![];
        }

        let fetch_queries: Vec<Query> = needs_fetch.iter().map(|(_, q)| q.clone()).collect();

        let keys: Vec<InFlightKey> = fetch_queries
            .iter()
            .map(|q| (q.clone(), Some(block)))
            .collect();
        let _guard = InFlightGuard::new(self.balance_cache.clone(), keys);

        let results = self.inner.get_balances(&fetch_queries, Some(block)).await;

        debug_assert_eq!(
            results.len(),
            fetch_queries.len(),
            "get_balances contract violation"
        );

        self.publish_in_flight_results(&fetch_queries, &results, Some(block));

        needs_fetch.into_iter().zip(results).collect()
    }

    fn publish_in_flight_results(
        &self,
        queries: &[Query],
        results: &[Result<U256>],
        block: Option<BlockNumber>,
    ) {
        self.with_cache_mut(|cache| {
            for (query, result) in queries.iter().zip(results.iter()) {
                cache.store_in_flight_result(query, block, result);
            }
        });
    }

    async fn wait_for_in_flight(&self, needs_wait: InFlightWaitList) -> Vec<(usize, Result<U256>)> {
        if needs_wait.is_empty() {
            return vec![];
        }

        let futures = needs_wait
            .into_iter()
            .map(|(i, _query, mut receiver)| async move {
                let result = self.await_in_flight_result(&mut receiver).await;
                (i, result)
            });

        futures::future::join_all(futures).await
    }

    async fn await_in_flight_result(&self, receiver: &mut InFlightResult) -> Result<U256> {
        let wait_result = receiver.wait_for(|opt| opt.is_some()).await;

        if wait_result.is_err() {
            return Err(anyhow::anyhow!("in-flight request cancelled"));
        }

        drop(wait_result);

        let shared_result = receiver
            .borrow_and_update()
            .as_ref()
            .expect("value must be Some after wait_for")
            .clone();

        match shared_result {
            Ok(balance) => Ok(balance),
            Err(err) => Err(anyhow::anyhow!("{:#}", err)),
        }
    }

    fn merge_in_flight_results(
        &self,
        total_len: usize,
        cached: Vec<(usize, Result<U256>)>,
        missing: &[usize],
        fetched_results: Vec<((usize, Query), Result<U256>)>,
        waited_results: Vec<(usize, Result<U256>)>,
    ) -> Vec<Result<U256>> {
        let mut results: Vec<Option<Result<U256>>> = (0..total_len).map(|_| None).collect();

        for (i, result) in cached {
            results[i] = Some(result);
        }

        for ((i, _), result) in fetched_results {
            results[missing[i]] = Some(result);
        }

        for (i, result) in waited_results {
            results[missing[i]] = Some(result);
        }

        results
            .into_iter()
            .map(|r| r.expect("all indices must be filled"))
            .collect()
    }

    async fn fetch_and_merge(
        &self,
        missing_queries: &[Query],
        missing: &[usize],
        cached: Vec<(usize, Result<U256>)>,
        total_len: usize,
    ) -> Vec<Result<U256>> {
        let fetch_block = self.with_cache(|cache| cache.current_block).unwrap_or(0);

        let new_balances = self
            .inner
            .get_balances(missing_queries, Some(fetch_block))
            .await;

        debug_assert_eq!(
            new_balances.len(),
            missing_queries.len(),
            "get_balances contract violation"
        );

        self.apply_fetch_results(&new_balances, missing_queries, fetch_block);

        let mut results: Vec<Option<Result<U256>>> = (0..total_len).map(|_| None).collect();

        for (i, result) in cached {
            results[i] = Some(result);
        }

        for (i, result) in missing.iter().zip(new_balances) {
            results[*i] = Some(result);
        }

        results
            .into_iter()
            .map(|r| r.expect("all indices must be filled"))
            .collect()
    }

    pub fn spawn_background_task(&self, block_stream: CurrentBlockWatcher) {
        let inner = self.inner.clone();
        let cache = self.balance_cache.clone();
        let mut stream = into_stream(block_stream);

        let task = async move {
            while let Some(block) = stream.next().await {
                let balances_to_update = {
                    let mut cache = match cache.lock() {
                        Ok(guard) => guard,
                        Err(err) => {
                            tracing::error!(
                                block = block.number,
                                error = %err,
                                "cache mutex poisoned in background task, skipping block update"
                            );
                            continue;
                        }
                    };
                    cache.current_block = block.number;
                    let policy = cache.policy;
                    cache
                        .data
                        .iter()
                        .filter_map(|(query, entry)| {
                            entry
                                .should_retain(block.number, &policy)
                                .then_some(query.clone())
                        })
                        .collect_vec()
                };

                let results = if !balances_to_update.is_empty() {
                    Some(
                        inner
                            .get_balances(&balances_to_update, Some(block.number))
                            .await,
                    )
                } else {
                    None
                };

                let mut cache = match cache.lock() {
                    Ok(guard) => guard,
                    Err(err) => {
                        tracing::error!(
                            block = block.number,
                            error = %err,
                            "cache mutex poisoned in background task, skipping balance updates"
                        );
                        continue;
                    }
                };
                if let Some(results) = results {
                    for (query, result) in balances_to_update.into_iter().zip(results) {
                        match result {
                            Ok(balance) => {
                                cache.update_balance(&query, balance, block.number);
                            }
                            Err(err) => {
                                tracing::warn!(
                                    ?query,
                                    error = ?err,
                                    block = block.number,
                                    "background balance update failed"
                                );
                            }
                        }
                    }
                }
                cache.cleanup_stale_entries();
            }
            tracing::error!("block stream terminated unexpectedly");
        };
        tokio::spawn(task.instrument(tracing::info_span!("balance_cache")));
    }
}

#[async_trait::async_trait]
impl BalanceFetching for Balances {
    #[instrument(skip_all)]
    async fn get_balances(
        &self,
        queries: &[Query],
        block_number: Option<BlockNumber>,
    ) -> Vec<Result<U256>> {
        match block_number {
            Some(bn) => self.handle_block_pinned_request(queries, bn).await,
            None => self.handle_none_request(queries).await,
        }
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
        crate::MockBalanceFetching,
        alloy_primitives::Address,
        ethrpc::block_stream::BlockInfo,
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

    impl CachePolicy {
        fn eviction_time_for_test(&self) -> BlockNumber {
            self.eviction_time()
        }
    }

    #[tokio::test]
    async fn caches_ok_results() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries, block| queries == [query(1)] && *block == Some(0))
            .returning(|_, _| vec![Ok(U256::ONE)]);

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        let result = fetcher.get_balances(&[query(1)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
        // Fetches balance from cache and skips calling `inner`.
        let result = fetcher.get_balances(&[query(1)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn does_not_cache_errors() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(2)
            .withf(|queries, block| queries == [query(1)] && *block == Some(0))
            .returning(|_, _| vec![Err(anyhow::anyhow!("some error"))]);

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner`.
        assert!(fetcher.get_balances(&[query(1)], None).await[0].is_err());
        // 2nd call to `inner`.
        assert!(fetcher.get_balances(&[query(1)], None).await[0].is_err());
    }

    #[tokio::test]
    async fn background_task_updates_cache_on_new_block() {
        let first_block = BlockInfo::default();
        let (sender, receiver) = tokio::sync::watch::channel(first_block);

        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries, block| queries == [query(1)] && *block == Some(0))
            .returning(|_, _| vec![Ok(U256::ONE)]);
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries, block| queries == [query(1)] && *block == Some(1))
            .returning(|_, _| vec![Ok(U256::ONE)]);

        let fetcher = Balances::new(Arc::new(inner));
        fetcher.spawn_background_task(receiver);

        let result = fetcher.get_balances(&[query(1)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        sender
            .send(BlockInfo {
                number: 1,
                ..Default::default()
            })
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        {
            let cache = fetcher.balance_cache.lock().unwrap();
            let entry = cache.data.get(&query(1)).unwrap();
            assert_eq!(entry.balance, U256::ONE);
            assert_eq!(entry.updated_block, 1);
        }

        let result = fetcher.get_balances(&[query(1)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
    }

    #[tokio::test]
    async fn can_return_new_and_cached_results_in_same_call() {
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries, block| queries == [query(1)] && *block == Some(0))
            .returning(|_, _| vec![Ok(U256::ONE)]);
        inner
            .expect_get_balances()
            .times(1)
            .withf(|queries, block| queries == [query(2)] && *block == Some(0))
            .returning(|_, _| vec![Ok(U256::from(2))]);

        let fetcher = Balances::new(Arc::new(inner));
        // 1st call to `inner` putting balance 1 into the cache.
        let result = fetcher.get_balances(&[query(1)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        // Fetches balance 1 from cache and balance 2 fresh. (2nd call to `inner`)
        let result = fetcher.get_balances(&[query(1), query(2)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);
        assert_eq!(result[1].as_ref().unwrap(), &U256::from(2));

        // Now balance 2 is also in the cache. Skipping call to `inner`.
        let result = fetcher.get_balances(&[query(2)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::from(2));
    }

    #[tokio::test]
    async fn unused_balances_get_evicted() {
        let first_block = BlockInfo::default();
        let (sender, receiver) = tokio::sync::watch::channel(first_block);

        let policy = CachePolicy::default();
        let mut inner = MockBalanceFetching::new();
        inner
            .expect_get_balances()
            .times(6)
            .returning(|_, _| vec![Ok(U256::ONE)]);

        let fetcher = Balances::with_policy(Arc::new(inner), policy);
        fetcher.spawn_background_task(receiver);

        let cached_entry = || {
            let cache = fetcher.balance_cache.lock().unwrap();
            cache.data.get(&query(1)).cloned()
        };

        assert!(cached_entry().is_none());
        // 1st call to `inner`. Balance gets cached.
        let result = fetcher.get_balances(&[query(1)], None).await;
        assert_eq!(result[0].as_ref().unwrap(), &U256::ONE);

        for block in 1..=policy.eviction_time_for_test() + 1 {
            assert!(cached_entry().is_some());
            // New block gets detected.
            sender
                .send(BlockInfo {
                    number: block,
                    ..Default::default()
                })
                .unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert!(cached_entry().is_none());
    }
}
