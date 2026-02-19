use {
    crate::maintenance::Maintaining,
    alloy::{
        primitives::{Address, B256},
        providers::{DynProvider, Provider},
        rpc::types::{Filter, Log},
        sol_types::SolEventInterface,
    },
    anyhow::{Context, Result},
    ethrpc::block_stream::{BlockNumberHash, BlockRetrieving, RangeInclusive},
    futures::{Stream, StreamExt, future},
    std::{pin::Pin, sync::Arc},
    tokio::sync::Mutex,
    tracing::{Instrument, instrument},
};

// We expect that there is never a reorg that changes more than the last n
// blocks.
pub const MAX_REORG_BLOCK_COUNT: u64 = 64;
// Saving events, we process at most this many at a time.
const INSERT_EVENT_BATCH_SIZE: usize = 10_000;
// MAX_BLOCKS_QUERIED is bigger than MAX_REORG_BLOCK_COUNT to increase the
// chances of avoiding the need for history fetch of block events, since history
// fetch is less efficient than latest block fetch
const MAX_BLOCKS_QUERIED: u64 = 2 * MAX_REORG_BLOCK_COUNT;
// Max number of rpc calls that can be sent at the same time to the node.
const MAX_PARALLEL_RPC_CALLS: usize = 128;

/// General idea behind the algorithm:
/// 1. Use `last_handled_blocks` as an indicator of the beginning of the block
///    crange that needs to be updated in current iteration. If it is empty,
///    means we need to check the storage, and if there are events in the
///    storage, continue from the last event block, if no events, do a full
///    reindexing from block 0.
///
/// 2. Define range of blocks that make sure no gaps or missed blocks exist.
/// 3. If this range is too big, split it into two subranges, one to update the
///    deep history blocks, second one to update the latest blocks (last X
///    canonical blocks) 4. Do the history update, and if successful, update
///    `last_handled_blocks` to make sure the data is consistent. 5. If history
///    update is successful, proceed with latest update, and if successful
///    update `last_handled_blocks`.
pub struct EventHandler<C, S, E>
where
    C: EventRetrieving<Event = E>,
    S: EventStoring<E>,
{
    block_retriever: Arc<dyn BlockRetrieving>,
    contract: C,
    store: S,
    last_handled_blocks: Vec<BlockNumberHash>,
    _phantom: std::marker::PhantomData<E>,
}

/// `EventStoring` is used by `EventHandler` for the purpose of giving the user
/// freedom in how, where and which events are stored.
///
/// # Examples
/// Databases: might transform, filter and classify which events are inserted
/// HashSet: For less persistent (in memory) storing, insert events into a set.
#[async_trait::async_trait]
pub trait EventStoring<E>: Send + Sync {
    /// Returns ok, on successful execution, otherwise an appropriate error
    ///
    /// # Arguments
    /// * `events` the contract events to be replaced by the implementer
    /// * `range` indicates a particular range of blocks on which to operate.
    async fn replace_events(&mut self, events: Vec<E>, range: RangeInclusive<u64>) -> Result<()>;

    /// Returns ok, on successful execution, otherwise an appropriate error
    ///
    /// # Arguments
    /// * `events` the contract events to be appended by the implementer
    async fn append_events(&mut self, events: Vec<E>) -> Result<()>;

    /// Fetches the last processed block to know where to resume indexing after
    /// a restart.
    async fn last_event_block(&self) -> Result<u64>;

    /// Stores the last processed block to know where to resume indexing after a
    /// restart.
    async fn persist_last_indexed_block(&mut self, last_block: u64) -> Result<()>;
}

pub type EventStream<T> = Pin<Box<dyn Stream<Item = Result<T>> + Send>>;

pub trait AlloyEventRetrieving {
    type Event: SolEventInterface + Send + Sync + 'static;

    /// Returns a filter that is used to query for the events of interest.
    fn filter(&self) -> Filter;

    fn provider(&self) -> &DynProvider;
}

/// Common `alloy` crate-related event retrieval patterns are extracted to
/// this trait to avoid code duplication in implementations.
#[async_trait::async_trait]
impl<T> EventRetrieving for T
where
    T: AlloyEventRetrieving + Send + Sync,
{
    type Event = (T::Event, Log);

    async fn get_events_by_block_hash(&self, block_hash: B256) -> Result<Vec<Self::Event>> {
        let filter = self.filter().at_block_hash(block_hash);
        let events = self
            .provider()
            .get_logs(&filter)
            .await
            .map_err(anyhow::Error::from)?
            .into_iter()
            .filter_map(|log| {
                let event = T::Event::decode_log(&log.inner).ok()?;
                Some((event.data, log))
            })
            .collect();

        Ok(events)
    }

    // Unfortunately, alloy's `watch_logs` does not support pagination yet, so it
    // is implemented manually here
    async fn get_events_by_block_range(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> Result<EventStream<Self::Event>> {
        const CHUNK_SIZE: usize = 500;

        let start = *block_range.start();
        let end = *block_range.end();
        let provider = self.provider().clone();
        let base_filter = self.filter();

        let stream = futures::stream::iter(start..=end)
            .chunks(CHUNK_SIZE)
            .then(move |range| {
                let provider = provider.clone();
                let filter = base_filter.clone();

                async move {
                    let Some((start, end)) = range.first().zip(range.last()) else {
                        return Ok(Vec::new());
                    };
                    let filter = filter.from_block(*start).to_block(*end);
                    let logs = provider.get_logs(&filter).await.with_context(|| {
                        format!("unable to get logs for blocks range {}-{}", start, end)
                    })?;

                    let events: Vec<Result<_>> = logs
                        .into_iter()
                        .map(|log| {
                            T::Event::decode_log(&log.inner)
                                .with_context(|| format!("unable to parse log: {:?}", log))
                                .map(|event| (event.data, log))
                        })
                        .collect();

                    Ok(events)
                }
            })
            .flat_map(|chunk_result| {
                futures::stream::iter(match chunk_result {
                    Ok(events) => events,
                    Err(e) => vec![Err(e)],
                })
            });

        Ok(Box::pin(stream))
    }

    fn address(&self) -> Vec<Address> {
        self.filter().address.iter().copied().collect()
    }
}

#[async_trait::async_trait]
pub trait EventRetrieving {
    type Event;

    async fn get_events_by_block_hash(&self, block_hash: B256) -> Result<Vec<Self::Event>>;

    async fn get_events_by_block_range(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> Result<EventStream<Self::Event>>;

    fn address(&self) -> Vec<Address>;
}

#[derive(Debug)]
struct EventRange {
    /// Optional block number range for fetching reorg safe history
    history_range: Option<RangeInclusive<u64>>,
    /// List of block numbers with hashes for fetching reorg unsafe blocks
    latest_blocks: Vec<BlockNumberHash>,
    /// Defines if reorg happened for reorg unsafe blocks
    is_reorg: bool,
}

impl<C, S, E> EventHandler<C, S, E>
where
    C: EventRetrieving<Event = E>,
    S: EventStoring<E>,
{
    pub fn new(
        block_retriever: Arc<dyn BlockRetrieving>,
        contract: C,
        store: S,
        start_sync_at_block: Option<BlockNumberHash>,
    ) -> Self {
        Self {
            block_retriever,
            contract,
            store,
            last_handled_blocks: {
                match start_sync_at_block {
                    Some(block) => vec![block],
                    None => vec![],
                }
            },
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new instance of the event handler that does not index events
    /// appearing in blocks before the specified input date. Note that this
    /// is a different behavior compared to [`Self::new()`]: that function
    /// always restarts indexing from the specified input block on creation;
    /// this function only indexes from the specified input block
    /// if there are no more recent events in the database.
    pub async fn new_skip_blocks_before(
        block_retriever: Arc<dyn BlockRetrieving>,
        contract: C,
        store: S,
        skip_blocks_before: BlockNumberHash,
    ) -> Result<Self> {
        let last_handled_block = store.last_event_block().await?;
        Ok(Self::new(
            block_retriever,
            contract,
            store,
            if last_handled_block >= skip_blocks_before.0 {
                None
            } else {
                Some(skip_blocks_before)
            },
        ))
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn last_handled_block(&self) -> Option<BlockNumberHash> {
        self.last_handled_blocks.last().cloned()
    }

    /// Defines block range, for which events should be fetched
    #[instrument(skip_all)]
    async fn event_block_range(&self) -> Result<EventRange> {
        let handled_blocks = if self.last_handled_blocks.is_empty() {
            let last_handled_block = self.store.last_event_block().await?;
            self.block_retriever
                .blocks(RangeInclusive::try_new(
                    last_handled_block,
                    last_handled_block,
                )?)
                .await?
        } else {
            self.last_handled_blocks.clone()
        };

        let current_block = self.block_retriever.current_block().await?;
        let current_block_number = current_block.number;
        let current_block_hash = current_block.hash;
        let (last_handled_block_number, last_handled_block_hash) = *handled_blocks.last().unwrap();
        tracing::debug!(
            "current block: {} - {:?}, handled_blocks: ({:?} - {:?})",
            current_block_number,
            current_block_hash,
            handled_blocks.first().map(|b| b.0),
            handled_blocks.last().map(|b| b.0),
        );

        // handle special case which happens most of the time (no reorg, just one new
        // block is added)
        if current_block.parent_hash == last_handled_block_hash {
            return Ok(EventRange {
                history_range: None,
                latest_blocks: vec![(current_block_number, current_block_hash)],
                is_reorg: false,
            });
        }

        // handle special case when no new block is added
        // this case would be caught later in algorithm by `detect_reorg_path`,
        // but we skip some node calls by returning early
        if (current_block_number, current_block_hash)
            == (last_handled_block_number, last_handled_block_hash)
        {
            return Ok(EventRange {
                history_range: None,
                latest_blocks: vec![],
                is_reorg: false,
            });
        }

        // Special case where multiple new blocks were added and no reorg happened.
        // Because we need to fetch the full block range we only do this if the number
        // of new blocks is sufficiently small.
        if let Ok(block_range) =
            RangeInclusive::try_new(last_handled_block_number, current_block_number)
            && block_range.end() - block_range.start() <= MAX_REORG_BLOCK_COUNT
        {
            let mut new_blocks = self.block_retriever.blocks(block_range).await?;
            if new_blocks.first().map(|b| b.1) == Some(last_handled_block_hash) {
                // first block is not actually new and was only fetched to detect a reorg
                new_blocks.remove(0);
                tracing::debug!(
                    first_new=?new_blocks.first(),
                    last_new=?new_blocks.last(),
                    "multiple new blocks without reorg"
                );
                return Ok(EventRange {
                    history_range: None,
                    latest_blocks: new_blocks,
                    is_reorg: false,
                });
            }
        }

        // full range of blocks which are considered for event update
        let block_range = RangeInclusive::try_new(
            last_handled_block_number.saturating_sub(MAX_REORG_BLOCK_COUNT),
            current_block_number,
        )?;

        let (history_range, latest_range) = split_range(block_range);
        tracing::debug!(
            "history range {:?}, latest_range {:?}",
            history_range,
            latest_range
        );

        let latest_blocks = self.block_retriever.blocks(latest_range).await?;
        tracing::debug!(
            "latest blocks: {:?} - {:?}",
            latest_blocks.first(),
            latest_blocks.last(),
        );

        // do not try to shorten the latest_blocks list if history range exists
        // if history range exists then we want to update for the full range of blocks,
        // otherwise history_blocks update would erase all subsequent blocks and we
        // might have a gap in storage
        let (latest_blocks, is_reorg) = match history_range {
            Some(_) => (latest_blocks, true),
            None => {
                let (latest_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
                (latest_blocks.to_vec(), is_reorg)
            }
        };

        tracing::debug!(
            "final latest blocks {:?} - {:?}, is reorg: {}",
            latest_blocks.first(),
            latest_blocks.last(),
            is_reorg
        );

        Ok(EventRange {
            history_range,
            latest_blocks,
            is_reorg,
        })
    }

    /// Get new events from the contract and insert them into the database.
    #[instrument(skip_all)]
    pub async fn update_events(&mut self) -> Result<()> {
        let event_range = self.event_block_range().await?;

        if let Some(range) = event_range.history_range {
            self.update_events_from_old_blocks(range).await?;
        }
        if let Some(last_block) = event_range.latest_blocks.last() {
            self.update_events_from_latest_blocks(&event_range.latest_blocks, event_range.is_reorg)
                .await?;
            self.store_mut()
                .persist_last_indexed_block(last_block.0)
                .await?;
        }
        Ok(())
    }

    #[instrument(skip_all)]
    async fn update_events_from_old_blocks(&mut self, range: RangeInclusive<u64>) -> Result<()> {
        // first get the blocks needed to update `last_handled_blocks` because if it
        // fails, it's safer to fail at the beginning of the function before we
        // update Storage
        let blocks = self
            .block_retriever
            .blocks(RangeInclusive::try_new(
                range.end().saturating_sub(MAX_REORG_BLOCK_COUNT),
                *range.end(),
            )?)
            .await?;

        let events = self
            .past_events_by_block_number_range(&range)
            .await
            .context("failed to get past events")?
            .chunks(INSERT_EVENT_BATCH_SIZE)
            .map(|chunk| chunk.into_iter().collect::<Result<Vec<_>, _>>());
        futures::pin_mut!(events);
        // We intentionally do not go with the obvious approach of deleting old events
        // first and then inserting new ones. Instead, we make sure that the
        // deletion and the insertion of the first batch of events happen in one
        // transaction. This is important for two reasons:
        // 1. It ensures that we only delete if we really have new events. Otherwise if
        // fetching new    events from the node fails for whatever reason we
        // might keep deleting events over and    over without inserting new
        // ones resulting in the database table getting cleared.    Note that we
        // do want to delete events if the new events are empty while fetching was
        //    successful.
        // 2. It ensures that other users of the database are unlikely to see an
        // inconsistent state    some events have been deleted but new ones not
        // yet inserted. This is important in case    for example another part
        // of the code calculates the total executed amount of an order.
        //    If this happened right after deletion but before insertion, then the
        // result would be    wrong. In theory this could still happen if the
        // last MAX_REORG_BLOCK_COUNT blocks had    more than
        // INSERT_TRADE_BATCH_SIZE trade events but this is unlikely.
        // There alternative solutions for 2. but this one is the most practical. For
        // example, we could keep all reorg-able events in this struct and only
        // store ones that are older than MAX_REORG_BLOCK_COUNT in the database
        // but then any code using trade events would have to go through this
        // class instead of being able to work with the database directly. Or we
        // could make the batch size unlimited but this runs into problems when we have
        // not updated it in a long time resulting in many missing events which
        // we would all have to in one transaction.
        let mut have_deleted_old_events = false;
        while let Some(events_chunk) = events.next().await {
            // Early return on error (through `?`) is important here so that the second
            // !have_deleted_old_events check (after the loop) is correct.
            let unwrapped_events = events_chunk.context("failed to get next chunk of events")?;
            if !have_deleted_old_events {
                self.store
                    .replace_events(unwrapped_events, range.clone())
                    .await?;
                have_deleted_old_events = true;
            } else {
                self.store.append_events(unwrapped_events).await?;
            };
        }
        // The `chunks` adaptor does not return an empty chunk if the stream was
        // completely empty. However we do want to delete old events in this
        // case as a rerorg might have removed events without adding new ones.
        if !have_deleted_old_events {
            self.store.replace_events(Vec::new(), range.clone()).await?;
        }

        self.update_last_handled_blocks(&blocks);
        Ok(())
    }

    #[instrument(skip_all)]
    async fn update_events_from_latest_blocks(
        &mut self,
        latest_blocks: &[BlockNumberHash],
        is_reorg: bool,
    ) -> Result<()> {
        debug_assert!(
            !latest_blocks.is_empty(),
            "entered update events with empty block list"
        );
        let (blocks, events) = self.past_events_by_block_hashes(latest_blocks).await;
        track_block_range(&format!("range_{}", blocks.len()));
        if blocks.is_empty() {
            return Err(anyhow::anyhow!(
                "no blocks to be updated - all filtered out"
            ));
        }

        // update storage regardless if it's a full update or partial update
        let range = RangeInclusive::try_new(blocks.first().unwrap().0, blocks.last().unwrap().0)?;
        if is_reorg {
            self.store.replace_events(events, range.clone()).await?;
        } else {
            self.store.append_events(events).await?;
        }
        self.update_last_handled_blocks(&blocks);

        // in case of partial update return error as an indicator that update did not
        // finish as expected either way we update partially to have the most
        // latest state in the storage in every moment
        if blocks != latest_blocks {
            tracing::debug!("partial update: {:?} - {:?}", blocks.first(), blocks.last());
            return Err(anyhow::anyhow!("update done partially"));
        }
        Ok(())
    }

    async fn past_events_by_block_hashes(
        &self,
        blocks: &[BlockNumberHash],
    ) -> (Vec<BlockNumberHash>, Vec<E>) {
        let (mut blocks_filtered, mut events) = (vec![], vec![]);
        for chunk in blocks.chunks(MAX_PARALLEL_RPC_CALLS) {
            for (i, result) in future::join_all(
                chunk
                    .iter()
                    .map(|block| self.contract.get_events_by_block_hash(block.1)),
            )
            .await
            .into_iter()
            .enumerate()
            {
                match result {
                    Ok(e) => {
                        if !e.is_empty() {
                            tracing::debug!(
                                "events fetched for block: {:?}, events: {}",
                                blocks[i],
                                e.len(),
                            );
                        }
                        blocks_filtered.push(blocks[i]);
                        events.extend(e);
                    }
                    Err(_) => return (blocks_filtered, events),
                }
            }
        }

        (blocks_filtered, events)
    }

    async fn past_events_by_block_number_range(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> Result<impl Stream<Item = Result<E>> + use<C, S, E> + Send> {
        self.contract.get_events_by_block_range(block_range).await
    }

    fn update_last_handled_blocks(&mut self, blocks: &[BlockNumberHash]) {
        tracing::debug!(
            "blocks to update into last_handled_blocks: {:?} - {:?}, last_handled_blocks: {:?} - \
             {:?}",
            blocks.first(),
            blocks.last(),
            self.last_handled_blocks.first(),
            self.last_handled_blocks.last(),
        );
        if blocks.is_empty() {
            return;
        }
        // delete forked blocks
        self.last_handled_blocks
            .retain(|block| block.0 < blocks.first().unwrap().0);
        // append new canonical blocks
        self.last_handled_blocks.extend(blocks.iter());
        // cap number of blocks to MAX_REORG_BLOCK_COUNT
        let start_index = self
            .last_handled_blocks
            .len()
            .saturating_sub(MAX_REORG_BLOCK_COUNT as usize);
        self.last_handled_blocks = self.last_handled_blocks[start_index..].to_vec();
        tracing::debug!(
            "last_handled_blocks after update: {:?} - {:?}",
            self.last_handled_blocks.first(),
            self.last_handled_blocks.last(),
        );
    }
}

/// Try to shorten the latest_blocks by detecting the reorg from previous event
/// update. If no reorg can be detected (for example, when `handled_blocks` is
/// shorter then the reorg depth) then fallback to full `latest_blocks` as a
/// safe measure.
fn detect_reorg_path<'a>(
    handled_blocks: &[BlockNumberHash],
    latest_blocks: &'a [BlockNumberHash],
) -> (&'a [BlockNumberHash], bool) {
    // in most cases, latest_blocks = handled_blocks + 1 newest block
    // therefore, is it more efficient to put the handled_blocks in outer loop,
    // so everything finishes in only two iterations.
    for handled_block in handled_blocks.iter().rev() {
        for (i, latest_block) in latest_blocks.iter().enumerate().rev() {
            if latest_block == handled_block {
                // found the same block in both lists, now we know the common ancestor, don't
                // include the ancestor
                let is_reorg = handled_block != handled_blocks.last().unwrap();
                return (&latest_blocks[i + 1..], is_reorg);
            }
        }
    }

    // reorg deeper than the EventHandler history (`handled_blocks`), return full
    // list
    let is_reorg = !handled_blocks.is_empty();
    (latest_blocks, is_reorg)
}

/// Splits range into two disjuctive consecutive ranges, second one containing
/// last (up to) MAX_BLOCKS_QUERIED elements, first one containing the rest (if
/// any)
fn split_range(range: RangeInclusive<u64>) -> (Option<RangeInclusive<u64>>, RangeInclusive<u64>) {
    let (start, end) = range.clone().into_inner();

    if end.saturating_sub(start) > MAX_BLOCKS_QUERIED {
        (
            Some(RangeInclusive::try_new(start, end - MAX_BLOCKS_QUERIED).unwrap()),
            RangeInclusive::try_new(end - MAX_BLOCKS_QUERIED + 1, end).unwrap(),
        )
    } else {
        (None, range)
    }
}

#[async_trait::async_trait]
impl<C, S, E> Maintaining for Mutex<EventHandler<C, S, E>>
where
    E: Send + Sync,
    C: EventRetrieving<Event = E> + Send + Sync,
    S: EventStoring<E> + Send + Sync,
{
    async fn run_maintenance(&self) -> Result<()> {
        let mut inner = self.lock().await;
        let address = inner.contract.address();
        inner
            .update_events()
            .instrument(tracing::info_span!("address", ?address))
            .await
    }

    fn name(&self) -> &str {
        "EventHandler"
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EventIndex {
    pub block_number: u64,
    pub log_index: u64,
}

impl EventIndex {
    pub fn new(block_number: u64, log_index: u64) -> Self {
        Self {
            block_number,
            log_index,
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "event_handler")]
struct Metrics {
    /// Tracks how many blocks were replaced/added in each call to EventHandler
    #[metric(labels("range"))]
    block_ranges: prometheus::IntCounterVec,
}

fn track_block_range(range: &str) {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
        .block_ranges
        .with_label_values(&[range])
        .inc();
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{eips::BlockNumberOrTag, primitives::b256},
        contracts::alloy::GPv2Settlement,
        ethrpc::{Web3, block_stream::block_number_to_block_number_hash},
    };

    impl AlloyEventRetrieving for GPv2Settlement::Instance {
        type Event = GPv2Settlement::GPv2Settlement::GPv2SettlementEvents;

        fn filter(&self) -> alloy::rpc::types::Filter {
            Filter::new().address(*self.address())
        }

        fn provider(&self) -> &alloy::providers::DynProvider {
            self.provider()
        }
    }

    /// Simple event storage for testing purposes of EventHandler
    struct EventStorage<T> {
        pub events: Vec<(T, alloy::rpc::types::Log)>,
    }

    #[async_trait::async_trait]
    impl<T> EventStoring<(T, alloy::rpc::types::Log)> for EventStorage<T>
    where
        T: Send + Sync,
    {
        async fn replace_events(
            &mut self,
            events: Vec<(T, alloy::rpc::types::Log)>,
            range: RangeInclusive<u64>,
        ) -> Result<()> {
            self.events
                .retain(|(_, log)| log.block_number.unwrap() < *range.start());
            self.append_events(events).await?;
            Ok(())
        }

        async fn append_events(&mut self, events: Vec<(T, alloy::rpc::types::Log)>) -> Result<()> {
            self.events.extend(events);
            Ok(())
        }

        async fn last_event_block(&self) -> Result<u64> {
            Ok(self
                .events
                .last()
                .map(|(_, log)| log.block_number.unwrap())
                .unwrap_or_default())
        }

        async fn persist_last_indexed_block(&mut self, _last_block: u64) -> Result<()> {
            // Nothing to do here since `last_event_block` looks up last stored event.
            Ok(())
        }
    }

    #[test]
    fn detect_reorg_path_test_both_empty() {
        let handled_blocks = vec![];
        let latest_blocks = vec![];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert!(replacement_blocks.is_empty());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_handled_blocks_empty() {
        let handled_blocks = vec![];
        let latest_blocks = vec![(1, B256::with_last_byte(1))];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks);
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_both_same() {
        // if the list are same, we return the common ancestor
        let handled_blocks = vec![(1, B256::with_last_byte(1))];
        let latest_blocks = vec![(1, B256::with_last_byte(1))];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert!(replacement_blocks.is_empty());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_common_case() {
        let handled_blocks = vec![(1, B256::with_last_byte(1)), (2, B256::with_last_byte(2))];
        let latest_blocks = vec![
            (1, B256::with_last_byte(1)),
            (2, B256::with_last_byte(2)),
            (3, B256::with_last_byte(3)),
            (4, B256::with_last_byte(4)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks[2..].to_vec());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_reorg_1() {
        let handled_blocks = vec![
            (1, B256::with_last_byte(1)),
            (2, B256::with_last_byte(2)),
            (3, B256::with_last_byte(3)),
        ];
        let latest_blocks = vec![
            (1, B256::with_last_byte(1)),
            (2, B256::with_last_byte(2)),
            (3, B256::with_last_byte(4)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks[2..].to_vec());
        assert!(is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_reorg_no_common_ancestor() {
        let handled_blocks = vec![(2, B256::with_last_byte(20))];
        let latest_blocks = vec![
            (1, B256::with_last_byte(11)),
            (2, B256::with_last_byte(21)),
            (3, B256::with_last_byte(31)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks);
        assert!(is_reorg);
    }

    #[test]
    fn split_range_test_equal() {
        let range = RangeInclusive::try_new(0, 0).unwrap();
        let (history_range, latest_range) = split_range(range.clone());
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_max_queries() {
        let range = RangeInclusive::try_new(0, MAX_BLOCKS_QUERIED).unwrap();
        let (history_range, latest_range) = split_range(range.clone());
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_max_queries_minus_one() {
        let range = RangeInclusive::try_new(0, MAX_BLOCKS_QUERIED - 1).unwrap();
        let (history_range, latest_range) = split_range(range.clone());
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_max_queries_plus_one() {
        let range = RangeInclusive::try_new(0, MAX_BLOCKS_QUERIED + 1).unwrap();
        let (history_range, latest_range) = split_range(range);
        assert_eq!(history_range, Some(RangeInclusive::try_new(0, 1).unwrap()));
        assert_eq!(
            latest_range,
            RangeInclusive::try_new(2, MAX_BLOCKS_QUERIED + 1).unwrap()
        );
    }

    #[tokio::test]
    #[ignore]
    async fn past_events_by_block_hashes_test() {
        let web3 = Web3::new_from_env();
        let contract = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let storage = EventStorage { events: vec![] };
        let blocks = vec![
            (
                15575559,
                b256!("0xa21ba3de6ac42185aa2b21e37cd63ff1572b763adff7e828f86590df1d1be118"),
            ),
            (
                15575560,
                b256!("0x5a737331194081e99b73d7a8b7a2ccff84e0aff39fa0e39aca0b660f3d6694c4"),
            ),
            (
                15575561,
                b256!(
                    "0xe91ec1a5a795c0739d99a60ac1df37cdf90b6c75c8150ace1cbad5b21f473b75" /* WRONG HASH! */
                ),
            ),
            (
                15575562,
                b256!("0xac1ca15622f17c62004de1f746728d4051103d8b7e558d39fd9fcec4d3348937"),
            ),
        ];
        let event_handler =
            EventHandler::new(Arc::new(web3.provider.clone()), contract, storage, None);
        let (replacement_blocks, _) = event_handler.past_events_by_block_hashes(&blocks).await;
        assert_eq!(replacement_blocks, blocks[..2]);
    }

    #[tokio::test]
    #[ignore]
    async fn update_events_test() {
        let web3 = Web3::new_from_env();
        let contract = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let storage = EventStorage { events: vec![] };
        let current_block = web3.provider.get_block_number().await.unwrap();

        const NUMBER_OF_BLOCKS: u64 = 300;

        //get block in history (current_block - NUMBER_OF_BLOCKS)
        let block = web3
            .provider
            .get_block_by_number(current_block.saturating_sub(NUMBER_OF_BLOCKS).into())
            .await
            .unwrap()
            .unwrap();
        let block = (block.number(), block.hash());
        let mut event_handler = EventHandler::new(
            Arc::new(web3.provider.clone()),
            contract,
            storage,
            Some(block),
        );
        let _result = event_handler.update_events().await;
        // add logs to event handler and observe
    }

    #[tokio::test]
    #[ignore]
    async fn multiple_new_blocks_but_no_reorg_test() {
        tracing_subscriber::fmt::init();
        let web3 = Web3::new_from_env();
        let contract = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();
        let storage: EventStorage<GPv2Settlement::GPv2Settlement::GPv2SettlementEvents> =
            EventStorage { events: vec![] };
        let current_block = web3.provider.get_block_number().await.unwrap();

        const NUMBER_OF_BLOCKS: u64 = 300;

        //get block in history (current_block - NUMBER_OF_BLOCKS)
        let block = web3
            .provider
            .get_block_by_number(current_block.saturating_sub(NUMBER_OF_BLOCKS).into())
            .await
            .unwrap()
            .unwrap();
        let block = (block.number(), block.hash());
        let mut event_handler = EventHandler::new(
            Arc::new(web3.provider.clone()),
            contract,
            storage,
            Some(block),
        );
        let _result = event_handler.update_events().await;
        tracing::info!("wait for at least 2 blocks to see if we hit the new code path");
        tokio::time::sleep(tokio::time::Duration::from_millis(26_000)).await;
        let _result = event_handler.update_events().await;
    }

    #[tokio::test]
    #[ignore]
    async fn optional_block_skipping() {
        let web3 = Web3::new_from_env();
        let contract = GPv2Settlement::Instance::deployed(&web3.provider)
            .await
            .unwrap();

        let current_block = web3.provider.get_block_number().await.unwrap();
        // In this test we query for events multiple times. Newer events might be
        // included each time we query again for the same events, but we want to
        // disregard them.
        let remove_events_after_test_start = |v: Vec<(
            GPv2Settlement::GPv2Settlement::GPv2SettlementEvents,
            alloy::rpc::types::Log,
        )>| {
            v.into_iter()
                .filter(|(_, log)| {
                    // We make the test robust against reorgs by removing events that are too new
                    log.block_number.unwrap() <= (current_block - MAX_REORG_BLOCK_COUNT)
                })
                .collect::<Vec<_>>()
        };

        // We expect that in the past ~24h intervals there have been two events in the
        // settlement contract that are at least MAX_REORG_BLOCK_COUNT apart.
        const RANGE_SIZE: u64 = 24 * 3600 / 12;

        let storage_empty = EventStorage { events: vec![] };
        let event_start = block_number_to_block_number_hash(
            &web3.provider,
            BlockNumberOrTag::Number(current_block - RANGE_SIZE),
        )
        .await
        .unwrap();
        let mut base_event_handler = EventHandler::new(
            Arc::new(web3.provider.clone()),
            contract.clone(),
            storage_empty,
            Some(event_start),
        );
        base_event_handler
            .update_events()
            .await
            .expect("Should update events");
        let base_all_events =
            remove_events_after_test_start(base_event_handler.store().events.clone());

        // We collect events again with an event handler generated from the same start
        // date but using `new_skip_blocks_before` if there are no events
        let storage_empty = EventStorage { events: vec![] };
        let event_start = block_number_to_block_number_hash(
            &web3.provider,
            BlockNumberOrTag::Number(current_block - RANGE_SIZE),
        )
        .await
        .unwrap();
        let mut base_block_skip_event_handler = EventHandler::new_skip_blocks_before(
            Arc::new(web3.provider.clone()),
            contract.clone(),
            storage_empty,
            event_start,
        )
        .await
        .expect("Should be able to create event handler");
        base_block_skip_event_handler
            .update_events()
            .await
            .expect("Should update events");
        let base_block_skip_all_events =
            remove_events_after_test_start(base_event_handler.store().events.clone());

        // No events already in storage means that we expect to have the same events
        // available
        assert_eq!(base_all_events, base_block_skip_all_events);

        // Events are ordered by date: first is oldest, last is most recent
        let first_event = base_all_events
            .first()
            .expect("Should have some events")
            .clone();
        let last_event = base_all_events
            .last()
            .expect("Should have some events")
            .clone();
        assert!(
            first_event.1.block_number.unwrap() + MAX_REORG_BLOCK_COUNT + 1
                < last_event.1.block_number.unwrap(),
            "Test assumption broken"
        );

        // Recreate the same event handler with the last event already in storage.
        let storage_nonempty = EventStorage {
            events: vec![last_event.clone()],
        };
        let mut nonempty_event_handler = EventHandler::new_skip_blocks_before(
            Arc::new(web3.provider.clone()),
            contract,
            storage_nonempty,
            // Same event start as for the two previous event handlers. The test checks that this
            // is disregarded.
            event_start,
        )
        .await
        .unwrap();
        nonempty_event_handler
            .update_events()
            .await
            .expect("Should update events");
        let nonempty_all_events =
            remove_events_after_test_start(nonempty_event_handler.store().events.clone());

        // Nonempty-storage event handler should not index all events, but all of them
        // should have been captured in (any of the) the event handlers that
        // started indexing earlier
        for event in nonempty_all_events.iter() {
            assert!(base_block_skip_all_events.contains(event));
        }
        // Also, older events shouldn't be available
        assert!(!nonempty_all_events.contains(&first_event))
        // However, some events slightly older than last_event's block might be
        // there because of reorg protection.
    }
}
