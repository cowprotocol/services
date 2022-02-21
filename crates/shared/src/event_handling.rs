use crate::{current_block::BlockRetrieving, maintenance::Maintaining};
use anyhow::{Context, Error, Result};
use ethcontract::contract::{AllEventsBuilder, ParseLog};
use ethcontract::errors::ExecutionError;
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

pub struct EventHandler<B, C, S>
where
    B: BlockRetrieving,
    C: EventRetrieving,
    S: EventStoring<C::Event>,
{
    block_retriever: B,
    contract: C,
    store: S,
    last_handled_block: Option<u64>,
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

    async fn last_event_block(&self) -> Result<u64>;
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
        start_sync_at_block: Option<u64>,
    ) -> Self {
        Self {
            block_retriever,
            contract,
            store,
            last_handled_block: start_sync_at_block,
        }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn last_handled_block(&self) -> Option<u64> {
        self.last_handled_block
    }

    async fn event_block_range(&self) -> Result<RangeInclusive<BlockNumber>> {
        // Instead of using only the most recent event block from the db we also store the last
        // handled block in self so that during long times of no events we do not query needlessly
        // large block ranges.
        let last_handled_block = match self.last_handled_block {
            Some(block) => block,
            None => self.store.last_event_block().await?,
        };
        let current_block = self.block_retriever.current_block_number().await?;
        let from_block = last_handled_block.saturating_sub(MAX_REORG_BLOCK_COUNT);
        anyhow::ensure!(
            from_block <= current_block,
            format!(
                "current block number according to node is {} which is more than {} blocks in the \
                 past compared to last handled block {}",
                current_block, MAX_REORG_BLOCK_COUNT, last_handled_block
            )
        );
        Ok(BlockNumber::Specific(from_block)..=BlockNumber::Latest(current_block))
    }

    /// Get new events from the contract and insert them into the database.
    pub async fn update_events(&mut self) -> Result<()> {
        let range = self.event_block_range().await?;
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
        self.last_handled_block = Some(range.end().to_u64());
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
    Specific(u64),
    Latest(u64),
}

impl BlockNumber {
    pub fn to_u64(self) -> u64 {
        match self {
            BlockNumber::Specific(block) => block,
            BlockNumber::Latest(block) => block,
        }
    }

    pub fn block_number(&self) -> Web3BlockNumber {
        match self {
            BlockNumber::Specific(block) => Web3BlockNumber::from(*block),
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
