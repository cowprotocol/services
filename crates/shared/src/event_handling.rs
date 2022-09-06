use crate::{current_block::BlockRetrieving, maintenance::Maintaining};
use anyhow::{ensure, Context, Error, Result};
use ethcontract::contract::{AllEventsBuilder, ParseLog};
use ethcontract::errors::ExecutionError;
use ethcontract::H256;
use ethcontract::{
    dyns::DynTransport, BlockNumber as Web3BlockNumber, Event as EthcontractEvent, EventMetadata,
};
use futures::{Stream, StreamExt, TryStreamExt};
use std::ops::RangeInclusive;
use tokio::sync::Mutex;

// We expect that there is never a reorg that changes more than the last n blocks.
pub const MAX_REORG_BLOCK_COUNT: u64 = 25;
// Saving events, we process at most this many at a time.
const INSERT_EVENT_BATCH_SIZE: usize = 10_000;

/// Block hash is optional since it is not always needed. For example, when we define a range of blocks,
/// block hash is important for range.end() block because that one is used to be stored in `last_handled_block`
/// and used in the next iterations, while for range.start() block is irrelevant, therefore, we don't want to spend
/// additional rpc calls just to satisfy the form.
pub type BlockNumberHash = (u64, Option<H256>);

pub struct EventHandler<B, C, S>
where
    B: BlockRetrieving,
    C: EventRetrieving,
    S: EventStoring<C::Event>,
{
    block_retriever: B,
    contract: C,
    store: S,
    last_handled_blocks: Vec<BlockNumberHash>,
}

/// `EventStoring` is used by `EventHandler` for the purpose of giving the user freedom
/// in how, where and which events are stored.
///
/// # Examples
/// Databases: might transform, filter and classify which events are inserted
/// HashSet: For less persistent (in memory) storing, insert events into a set.
#[async_trait::async_trait]
pub trait EventStoring<T>: Send + Sync {
    /// Returns ok, on successful execution, otherwise an appropriate error
    ///
    /// # Arguments
    /// * `events` the contract events to be replaced by the implementer
    /// * `range` indicates a particular range of blocks on which to operate.
    async fn replace_events(
        &mut self,
        events: Vec<EthcontractEvent<T>>,
        range: RangeInclusive<BlockNumber>,
    ) -> Result<()>;

    /// Returns ok, on successful execution, otherwise an appropriate error
    ///
    /// # Arguments
    /// * `events` the contract events to be appended by the implementer
    async fn append_events(&mut self, events: Vec<EthcontractEvent<T>>) -> Result<()>;

    async fn last_event_block(&self) -> Result<BlockNumberHash>;
}

pub trait EventRetrieving {
    type Event: ParseLog;
    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event>;
}

impl<B, C, S> EventHandler<B, C, S>
where
    B: BlockRetrieving,
    C: EventRetrieving,
    S: EventStoring<C::Event>,
{
    pub fn new(
        block_retriever: B,
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
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn last_handled_block(&self) -> Option<BlockNumberHash> {
        self.last_handled_blocks.last().cloned()
    }

    async fn event_block_range(
        &self,
    ) -> Result<(RangeInclusive<BlockNumber>, Vec<BlockNumberHash>)> {
        let current_block = self.block_retriever.current_block().await?;
        let handled_blocks = if self.last_handled_blocks.is_empty() {
            vec![self.store.last_event_block().await?]
        } else {
            self.last_handled_blocks.clone()
        };

        let current_block_number = current_block.number.context("missing number")?.as_u64();
        let last_handled_block_hash = handled_blocks.last().unwrap().1.context("missing hash")?;

        // handle special case which happens most of the time (no reorg, just one new block is added)
        if current_block.parent_hash == last_handled_block_hash {
            let current_block = (current_block_number, current_block.hash);
            return Ok((
                (BlockNumber::Latest(current_block)..=BlockNumber::Latest(current_block)),
                vec![current_block],
            ));
        }

        let current_blocks = self
            .block_retriever
            .current_blocks(current_block_number, MAX_REORG_BLOCK_COUNT)
            .await?;

        let block_range = detect_reorg_path(&current_blocks, &handled_blocks)?;

        anyhow::ensure!(
            block_range.start().0 <= block_range.end().0,
            format!(
                "current block number according to node is {} which is more than {} blocks in the \
                 past compared to last handled block {}",
                block_range.end().0,
                MAX_REORG_BLOCK_COUNT,
                block_range.start().0
            )
        );
        Ok((
            BlockNumber::Specific(*block_range.start())..=BlockNumber::Latest(*block_range.end()),
            current_blocks,
        ))
    }

    /// Get new events from the contract and insert them into the database.
    pub async fn update_events(&mut self) -> Result<()> {
        let (range, replacement_blocks) = self.event_block_range().await?;
        tracing::debug!("updating events in block range {:?}", range);
        let events = self
            .past_events(&range)
            .await
            .context("failed to get past events")?
            .chunks(INSERT_EVENT_BATCH_SIZE)
            .map(|chunk| chunk.into_iter().collect::<Result<Vec<_>, _>>());
        futures::pin_mut!(events);
        // We intentionally do not go with the obvious approach of deleting old events first and
        // then inserting new ones. Instead, we make sure that the deletion and the insertion of the
        // first batch of events happen in one transaction.
        // This is important for two reasons:
        // 1. It ensures that we only delete if we really have new events. Otherwise if fetching new
        //    events from the node fails for whatever reason we might keep deleting events over and
        //    over without inserting new ones resulting in the database table getting cleared.
        //    Note that we do want to delete events if the new events are empty while fetching was
        //    successful.
        // 2. It ensures that other users of the database are unlikely to see an inconsistent state
        //    some events have been deleted but new ones not yet inserted. This is important in case
        //    for example another part of the code calculates the total executed amount of an order.
        //    If this happened right after deletion but before insertion, then the result would be
        //    wrong. In theory this could still happen if the last MAX_REORG_BLOCK_COUNT blocks had
        //    more than INSERT_TRADE_BATCH_SIZE trade events but this is unlikely.
        // There alternative solutions for 2. but this one is the most practical. For example, we
        // could keep all reorg-able events in this struct and only store ones that are older than
        // MAX_REORG_BLOCK_COUNT in the database but then any code using trade events would have to
        // go through this class instead of being able to work with the database directly.
        // Or we could make the batch size unlimited but this runs into problems when we have not
        // updated it in a long time resulting in many missing events which we would all have to
        // in one transaction.
        let mut have_deleted_old_events = false;
        while let Some(events_chunk) = events.next().await {
            // Early return on error (through `?`) is important here so that the second
            // !have_deleted_old_eventsS check (after the loop) is correct.
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
        // The `chunks` adaptor does not return an empty chunk if the stream was completely empty.
        // However we do want to delete old events in this case as a rerorg might have removed
        // events without adding new ones.
        if !have_deleted_old_events {
            self.store.replace_events(Vec::new(), range.clone()).await?;
        }

        track_block_range(&format!("range_{}", replacement_blocks.len()));
        if !replacement_blocks.is_empty() {
            // delete forked blocks
            self.last_handled_blocks
                .retain(|block| block.0 < replacement_blocks.first().unwrap().0);
            // append new canonical blocks
            self.last_handled_blocks
                .extend(replacement_blocks.into_iter());
            // cap number of blocks to MAX_REORG_BLOCK_COUNT
            let start_index = self
                .last_handled_blocks
                .len()
                .saturating_sub(MAX_REORG_BLOCK_COUNT as usize);
            self.last_handled_blocks =
                self.last_handled_blocks[start_index..self.last_handled_blocks.len()].to_vec();
        }
        Ok(())
    }

    async fn past_events(
        &self,
        block_range: &RangeInclusive<BlockNumber>,
    ) -> Result<impl Stream<Item = Result<EthcontractEvent<C::Event>>>, ExecutionError> {
        Ok(self
            .contract
            .get_events()
            .from_block((*block_range.start()).block_number())
            .to_block((*block_range.end()).block_number())
            .block_page_size(500)
            .query_paginated()
            .await?
            .map_err(Error::from))
    }
}

fn detect_reorg_path(
    handled_blocks: &Vec<BlockNumberHash>,
    current_blocks: &Vec<BlockNumberHash>,
) -> Result<RangeInclusive<BlockNumberHash>> {
    ensure!(!handled_blocks.is_empty() && !current_blocks.is_empty());

    // in most cases, current_blocks = handled_blocks + 1 newest block
    // therefore, is it more efficient to put the handled_blocks in outer loop,
    // so everything finishes in only two iterations.
    for handled_block in handled_blocks.iter().rev() {
        for current_block in current_blocks.iter().rev() {
            if current_block.0 == handled_block.0 && current_block.1 == handled_block.1 {
                // found the same block in both lists, now we know the common ancestor
                return Ok(*current_block..=*current_blocks.last().unwrap());
            }
        }
    }

    //cant figure out the reorg, fallback to regular 25 blocks reorg
    Ok((
        handled_blocks
            .last()
            .unwrap()
            .0
            .saturating_sub(MAX_REORG_BLOCK_COUNT),
        None,
    )..=*current_blocks.last().unwrap())
}

#[async_trait::async_trait]
impl<B, C, S> Maintaining for Mutex<EventHandler<B, C, S>>
where
    B: BlockRetrieving + Send + Sync,
    C: EventRetrieving + Send + Sync,
    C::Event: Send,
    S: EventStoring<C::Event> + Send + Sync,
{
    async fn run_maintenance(&self) -> Result<()> {
        self.lock().await.update_events().await
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

impl From<&EventMetadata> for EventIndex {
    fn from(meta: &EventMetadata) -> Self {
        EventIndex {
            block_number: meta.block_number,
            log_index: meta.log_index as u64,
        }
    }
}

// Helper type around the Web3BlockNumber that allows us to specify `BlockNumber::Latest` for range queries
// while still storing concrete block numbers for latest internally. The issue with concrete block numbers for
// range queries is that e.g. behind a load balancer node A might not yet have seen the block number another
// node B considers to be Latest.
// Given our reorg-tolerant query logic it's not a problem to store a concrete block number that is slightly
// off from the actually used Latest block number.
#[derive(Debug, Clone, Copy)]
pub enum BlockNumber {
    Specific(BlockNumberHash),
    Latest(BlockNumberHash),
}

impl BlockNumber {
    pub fn to_u64(self) -> u64 {
        match self {
            BlockNumber::Specific(block) => block.0,
            BlockNumber::Latest(block) => block.0,
        }
    }

    pub fn to_value(self) -> BlockNumberHash {
        match self {
            BlockNumber::Specific(block) => block,
            BlockNumber::Latest(block) => block,
        }
    }

    pub fn block_number(&self) -> Web3BlockNumber {
        match self {
            BlockNumber::Specific(block) => Web3BlockNumber::from(block.0),
            BlockNumber::Latest(_) => Web3BlockNumber::Latest,
        }
    }
}

#[macro_export]
macro_rules! impl_event_retrieving {
    ($vis:vis $name:ident for $($contract_module:tt)*) => {
        $vis struct $name($($contract_module)*::Contract);

        impl $crate::event_handling::EventRetrieving for $name {
            type Event = $($contract_module)*::Event;

            fn get_events(&self) -> ::ethcontract::contract::AllEventsBuilder<
                ::ethcontract::dyns::DynTransport,
                Self::Event,
            > {
                self.0.all_events()
            }
        }
    };
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "event_handler")]
struct Metrics {
    /// Tracks how many blocks were replaced/added in each call to EventHandler
    #[metric(labels("range"))]
    block_ranges: prometheus::IntCounterVec,
}

fn track_block_range(range: &str) {
    Metrics::instance(global_metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
        .block_ranges
        .with_label_values(&[range])
        .inc();
}
