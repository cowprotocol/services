use crate::{current_block::BlockRetrieving, maintenance::Maintaining};
use anyhow::{ensure, Context, Error, Result};
use ethcontract::contract::{AllEventsBuilder, ParseLog};
use ethcontract::errors::ExecutionError;
use ethcontract::H256;
use ethcontract::{dyns::DynTransport, Event as EthcontractEvent, EventMetadata};
use futures::{future, Stream, StreamExt, TryStreamExt};
use std::ops::RangeInclusive;
use tokio::sync::Mutex;

// We expect that there is never a reorg that changes more than the last n blocks.
pub const MAX_REORG_BLOCK_COUNT: u64 = 25;
// Saving events, we process at most this many at a time.
const INSERT_EVENT_BATCH_SIZE: usize = 10_000;
const MAX_BLOCKS_QUERIED: u64 = 2 * MAX_REORG_BLOCK_COUNT;

pub type BlockNumberHash = (u64, H256);

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
        range: RangeInclusive<u64>,
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

struct EventRange {
    /// Optional block number range for fetching reorg safe history
    history_range: Option<RangeInclusive<u64>>,
    /// List of block numbers with hashes for fetching reorg unsafe blocks
    latest_blocks: Vec<BlockNumberHash>,
    /// Defines if reorg happened for reorg unsafe blocks
    is_reorg: bool,
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

    /// Defines block range, for which events should be fetched
    async fn event_block_range(&self) -> Result<EventRange> {
        let handled_blocks = if self.last_handled_blocks.is_empty() {
            let last_handled_block = self.store.last_event_block().await?;
            self.block_retriever
                .blocks(RangeInclusive::new(last_handled_block, last_handled_block))
                .await?
        } else {
            self.last_handled_blocks.clone()
        };

        let current_block = self.block_retriever.current_block().await?;
        let current_block_number = current_block.number.context("missing number")?.as_u64();
        let current_block_hash = current_block.hash.context("missing hash")?;
        let (last_handled_block_number, last_handled_block_hash) = *handled_blocks.last().unwrap();
        tracing::debug!(
            "current block: {} - {:?}, handled_blocks: {:?}",
            current_block_number,
            current_block_hash,
            handled_blocks,
        );

        // handle special case which happens most of the time (no reorg, just one new block is added)
        if current_block.parent_hash == last_handled_block_hash {
            return Ok(EventRange {
                history_range: None,
                latest_blocks: vec![(current_block_number, current_block_hash)],
                is_reorg: false,
            });
        }

        // handle special case when no new block is added
        if (current_block_number, current_block_hash)
            == (last_handled_block_number, last_handled_block_hash)
        {
            return Ok(EventRange {
                history_range: None,
                latest_blocks: vec![],
                is_reorg: false,
            });
        }

        // full range of blocks which are considered for event update
        let block_range = RangeInclusive::new(
            last_handled_block_number.saturating_sub(MAX_REORG_BLOCK_COUNT),
            current_block_number,
        );
        ensure!(
            !block_range.is_empty(),
            "current block number according to node is {} which is more than {} blocks in the \
                 past compared to last handled block {}",
            block_range.end(),
            MAX_REORG_BLOCK_COUNT,
            block_range.start()
        );

        let (history_range, latest_range) = split_range(block_range)?;
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
        // otherwise history_blocks update would erase all subsequent blocks and we might have a gap in storage
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
    pub async fn update_events(&mut self) -> Result<()> {
        let event_range = self.event_block_range().await?;

        if let Some(range) = event_range.history_range {
            self.update_events_from_old_blocks(range).await?;
        }
        self.update_events_from_latest_blocks(&event_range.latest_blocks, event_range.is_reorg)
            .await?;
        Ok(())
    }

    async fn update_events_from_old_blocks(&mut self, range: RangeInclusive<u64>) -> Result<()> {
        let events = self
            .past_events_by_block_number_range(&range)
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
        // The `chunks` adaptor does not return an empty chunk if the stream was completely empty.
        // However we do want to delete old events in this case as a rerorg might have removed
        // events without adding new ones.
        if !have_deleted_old_events {
            self.store.replace_events(Vec::new(), range.clone()).await?;
        }

        let blocks = self
            .block_retriever
            .blocks(RangeInclusive::new(
                range.end().saturating_sub(MAX_REORG_BLOCK_COUNT),
                *range.end(),
            ))
            .await?;

        self.update_last_handled_blocks(&blocks);
        Ok(())
    }

    async fn update_events_from_latest_blocks(
        &mut self,
        blocks: &[BlockNumberHash],
        is_reorg: bool,
    ) -> Result<()> {
        let (blocks, events) = self.past_events_by_block_hashes(blocks).await;
        track_block_range(&format!("range_{}", blocks.len()));
        tracing::debug!(
            "final blocks for updating: {:?} - {:?}",
            blocks.first(),
            blocks.last()
        );
        if blocks.is_empty() {
            return Ok(());
        }
        let range = RangeInclusive::new(blocks.first().unwrap().0, blocks.last().unwrap().0);
        if is_reorg {
            self.store.replace_events(events, range.clone()).await?;
        } else {
            self.store.append_events(events).await?;
        }

        self.update_last_handled_blocks(&blocks);
        Ok(())
    }

    async fn past_events_by_block_hashes(
        &self,
        blocks: &[BlockNumberHash],
    ) -> (Vec<BlockNumberHash>, Vec<EthcontractEvent<C::Event>>) {
        let (mut blocks_filtered, mut events) = (vec![], vec![]);
        for (i, result) in future::join_all(
            blocks
                .iter()
                .map(|block| self.contract.get_events().block_hash(block.1).query()),
        )
        .await
        .into_iter()
        .enumerate()
        {
            match result {
                Ok(e) => {
                    blocks_filtered.push(blocks[i]);
                    events.extend(e.into_iter());
                }
                Err(_) => return (blocks_filtered, events),
            }
        }

        (blocks_filtered, events)
    }

    async fn past_events_by_block_number_range(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> Result<impl Stream<Item = Result<EthcontractEvent<C::Event>>>, ExecutionError> {
        Ok(self
            .contract
            .get_events()
            .from_block((*block_range.start()).into())
            .to_block((*block_range.end()).into())
            .block_page_size(500)
            .query_paginated()
            .await?
            .map_err(Error::from))
    }

    fn update_last_handled_blocks(&mut self, blocks: &[BlockNumberHash]) {
        tracing::debug!(
            "blocks to update into last_handled_blocks: {:?} - {:?}, last_handled_blocks: {:?} - {:?}",
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

/// Try to shorten the latest_blocks by detecting the reorg from previous event update.
/// If no reorg can be detected (for example, when `handled_blocks` is shorter then the
/// reorg depth) then fallback to full `latest_blocks` as a safe measure.
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
                // found the same block in both lists, now we know the common ancestor, don't include the ancestor
                let is_reorg = handled_block != handled_blocks.last().unwrap();
                return (&latest_blocks[i + 1..], is_reorg);
            }
        }
    }

    // reorg deeper than the EventHandler history (`handled_blocks`), return full list
    let is_reorg = !handled_blocks.is_empty();
    (latest_blocks, is_reorg)
}

/// Splits range into two disjuctive consecutive ranges, second one containing last (up to)
/// MAX_BLOCKS_QUERIED elements, first one containing the rest (if any)
fn split_range(
    range: RangeInclusive<u64>,
) -> Result<(Option<RangeInclusive<u64>>, RangeInclusive<u64>)> {
    ensure!(
        MAX_BLOCKS_QUERIED > 0,
        "MAX_BLOCKS_QUERIED must be greater than zero"
    );

    let start = range.start();
    let end = range.end();

    Ok(if end.saturating_sub(*start) > MAX_BLOCKS_QUERIED {
        (
            Some(RangeInclusive::new(*start, end - MAX_BLOCKS_QUERIED)),
            RangeInclusive::new(end - MAX_BLOCKS_QUERIED + 1, *end),
        )
    } else {
        (None, range)
    })
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

#[macro_export]
macro_rules! impl_event_retrieving {
    ($vis:vis $name:ident for $($contract_module:tt)*) => {
        $vis struct $name($($contract_module)*::Contract);

        impl $name {
            #[allow(dead_code)]
            pub fn new(instance: $($contract_module)*::Contract) -> Self {
                Self(instance)
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{transport::create_env_test_transport, Web3};
    use contracts::{gpv2_settlement, GPv2Settlement};
    use ethcontract::BlockNumber;
    use std::str::FromStr;

    impl_event_retrieving! {
        pub GPv2SettlementContract for gpv2_settlement
    }

    /// Simple event storage for testing purposes of EventHandler
    struct EventStorage<T> {
        events: Vec<EthcontractEvent<T>>,
    }

    #[async_trait::async_trait]
    impl<T> EventStoring<T> for EventStorage<T>
    where
        T: Send + Sync,
    {
        async fn replace_events(
            &mut self,
            events: Vec<EthcontractEvent<T>>,
            range: RangeInclusive<u64>,
        ) -> Result<()> {
            self.events
                .retain(|event| event.meta.clone().unwrap().block_number < *range.start());
            self.append_events(events).await?;
            Ok(())
        }

        async fn append_events(&mut self, events: Vec<EthcontractEvent<T>>) -> Result<()> {
            self.events.extend(events.into_iter());
            Ok(())
        }

        async fn last_event_block(&self) -> Result<u64> {
            Ok(self
                .events
                .last()
                .map(|event| event.meta.clone().unwrap().block_number)
                .unwrap_or_default())
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
        let latest_blocks = vec![(1, H256::from_low_u64_be(1))];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks);
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_both_same() {
        // if the list are same, we return the common ancestor
        let handled_blocks = vec![(1, H256::from_low_u64_be(1))];
        let latest_blocks = vec![(1, H256::from_low_u64_be(1))];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert!(replacement_blocks.is_empty());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_common_case() {
        let handled_blocks = vec![(1, H256::from_low_u64_be(1)), (2, H256::from_low_u64_be(2))];
        let latest_blocks = vec![
            (1, H256::from_low_u64_be(1)),
            (2, H256::from_low_u64_be(2)),
            (3, H256::from_low_u64_be(3)),
            (4, H256::from_low_u64_be(4)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks[2..].to_vec());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_reorg_1() {
        let handled_blocks = vec![
            (1, H256::from_low_u64_be(1)),
            (2, H256::from_low_u64_be(2)),
            (3, H256::from_low_u64_be(3)),
        ];
        let latest_blocks = vec![
            (1, H256::from_low_u64_be(1)),
            (2, H256::from_low_u64_be(2)),
            (3, H256::from_low_u64_be(4)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks[2..].to_vec());
        assert!(is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_reorg_no_common_ancestor() {
        let handled_blocks = vec![(2, H256::from_low_u64_be(20))];
        let latest_blocks = vec![
            (1, H256::from_low_u64_be(11)),
            (2, H256::from_low_u64_be(21)),
            (3, H256::from_low_u64_be(31)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &latest_blocks);
        assert_eq!(replacement_blocks, latest_blocks);
        assert!(is_reorg);
    }

    #[test]
    fn split_range_test_empty_range() {
        let range = RangeInclusive::new(1, 0);
        let (history_range, latest_range) = split_range(range.clone()).unwrap();
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_equal() {
        let range = RangeInclusive::new(0, 0);
        let (history_range, latest_range) = split_range(range.clone()).unwrap();
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_max_queries() {
        let range = RangeInclusive::new(0, MAX_BLOCKS_QUERIED);
        let (history_range, latest_range) = split_range(range.clone()).unwrap();
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_max_queries_minus_one() {
        let range = RangeInclusive::new(0, MAX_BLOCKS_QUERIED - 1);
        let (history_range, latest_range) = split_range(range.clone()).unwrap();
        assert!(history_range.is_none() && latest_range == range);
    }

    #[test]
    fn split_range_test_max_queries_plus_one() {
        let range = RangeInclusive::new(0, MAX_BLOCKS_QUERIED + 1);
        let (history_range, latest_range) = split_range(range).unwrap();
        assert_eq!(history_range, Some(RangeInclusive::new(0, 1)));
        assert_eq!(latest_range, RangeInclusive::new(2, 51));
    }

    #[tokio::test]
    #[ignore]
    async fn past_events_by_block_hashes_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let storage = EventStorage { events: vec![] };
        let blocks = vec![
            (
                15575559,
                H256::from_str(
                    "0xa21ba3de6ac42185aa2b21e37cd63ff1572b763adff7e828f86590df1d1be118",
                )
                .unwrap(),
            ),
            (
                15575560,
                H256::from_str(
                    "0x5a737331194081e99b73d7a8b7a2ccff84e0aff39fa0e39aca0b660f3d6694c4",
                )
                .unwrap(),
            ),
            (
                15575561,
                H256::from_str(
                    "0xe91ec1a5a795c0739d99a60ac1df37cdf90b6c75c8150ace1cbad5b21f473b75", //WRONG HASH!
                )
                .unwrap(),
            ),
            (
                15575562,
                H256::from_str(
                    "0xac1ca15622f17c62004de1f746728d4051103d8b7e558d39fd9fcec4d3348937",
                )
                .unwrap(),
            ),
        ];
        let event_handler =
            EventHandler::new(web3, GPv2SettlementContract(contract), storage, None);
        let (replacement_blocks, _) = event_handler.past_events_by_block_hashes(&blocks).await;
        assert_eq!(replacement_blocks, blocks[..2]);
    }

    #[tokio::test]
    #[ignore]
    async fn update_events_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
        let storage = EventStorage { events: vec![] };
        let current_block = web3.eth().block_number().await.unwrap();

        const NUMBER_OF_BLOCKS: u64 = 300;

        //get block in history (current_block - NUMBER_OF_BLOCKS)
        let block = web3
            .eth()
            .block(
                BlockNumber::Number(current_block.saturating_sub(NUMBER_OF_BLOCKS.into())).into(),
            )
            .await
            .unwrap()
            .unwrap();
        let block = (block.number.unwrap().as_u64(), block.hash.unwrap());
        let mut event_handler =
            EventHandler::new(web3, GPv2SettlementContract(contract), storage, Some(block));
        let _result = event_handler.update_events().await;
        // add logs to event handler and observe
    }
}
