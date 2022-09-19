use crate::{current_block::BlockRetrieving, maintenance::Maintaining};
use anyhow::{Context, Result};
use ethcontract::contract::{AllEventsBuilder, ParseLog};
use ethcontract::H256;
use ethcontract::{dyns::DynTransport, Event as EthcontractEvent, EventMetadata};
use std::ops::RangeInclusive;
use tokio::sync::Mutex;

// We expect that there is never a reorg that changes more than the last n blocks.
pub const MAX_REORG_BLOCK_COUNT: u64 = 25;

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

    async fn event_block_range(&self) -> Result<(Vec<BlockNumberHash>, bool)> {
        let current_block = self.block_retriever.current_block().await?;
        let handled_blocks = if self.last_handled_blocks.is_empty() {
            // since we don't want `Store` to be responsible for hashes, here we just get
            // block number and get the `safe` block from it - this is done only on first init
            let last_handled_block = self.store.last_event_block().await?;
            self.block_retriever
                .blocks(RangeInclusive::new(last_handled_block, last_handled_block))
                .await?
        } else {
            self.last_handled_blocks.clone()
        };

        let current_block_number = current_block.number.context("missing number")?.as_u64();
        let current_block_hash = current_block.hash.context("missing hash")?;
        let (last_handled_block_number, last_handled_block_hash) = *handled_blocks.last().unwrap();
        tracing::debug!(
            "current block: {} - {:?}, last_handled_blocks: {} - {:?}",
            current_block_number,
            current_block_hash,
            last_handled_block_number,
            last_handled_block_hash
        );

        // handle special case which happens most of the time (no reorg, just one new block is added)
        if current_block.parent_hash == last_handled_block_hash {
            let is_reorg = false;
            return Ok((vec![(current_block_number, current_block_hash)], is_reorg));
        }

        // handle special case when no new block is added
        if (current_block_number, current_block_hash)
            == (last_handled_block_number, last_handled_block_hash)
        {
            let is_reorg = false;
            return Ok((vec![], is_reorg));
        }

        // get all canonical blocks starting from a safe reorg block (last_handled_block_number-25)
        // and ending with a current canonical block number. This way, we make sure we have
        // all block numbers and hashes needed for updating the storage
        let current_blocks = self
            .block_retriever
            .blocks(RangeInclusive::new(
                last_handled_block_number.saturating_sub(MAX_REORG_BLOCK_COUNT),
                current_block_number,
            ))
            .await?;
        tracing::debug!(
            "current blocks: {:?} - {:?}",
            current_blocks.first().unwrap(),
            current_blocks.last().unwrap()
        );

        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        Ok((replacement_blocks.to_vec(), is_reorg))
    }

    /// Get new events from the contract and insert them into the database.
    pub async fn update_events(&mut self) -> Result<()> {
        let (replacement_blocks, is_reorg) = self.event_block_range().await?;
        tracing::debug!(
            "replacement_blocks before filtering: {:?}",
            replacement_blocks
        );
        let (replacement_blocks, events) = self.past_events(&replacement_blocks).await;
        track_block_range(&format!("range_{}", replacement_blocks.len()));
        tracing::debug!(
            "replacement_blocks after filtering: {:?}, events number: {}",
            replacement_blocks,
            events.len()
        );
        if replacement_blocks.is_empty() {
            return Ok(());
        }

        let range = RangeInclusive::new(
            replacement_blocks.first().unwrap().0,
            replacement_blocks.last().unwrap().0,
        );
        if is_reorg {
            self.store.replace_events(events, range.clone()).await?;
            // delete forked blocks
            self.last_handled_blocks
                .retain(|block| block.0 < replacement_blocks.first().unwrap().0);
        } else {
            self.store.append_events(events).await?;
        }

        // append new canonical blocks
        self.last_handled_blocks
            .extend(replacement_blocks.into_iter());
        // cap number of blocks to MAX_REORG_BLOCK_COUNT
        let start_index = self
            .last_handled_blocks
            .len()
            .saturating_sub(MAX_REORG_BLOCK_COUNT as usize);
        self.last_handled_blocks = self.last_handled_blocks[start_index..].to_vec();

        Ok(())
    }

    async fn past_events(
        &self,
        blocks: &[BlockNumberHash],
    ) -> (Vec<BlockNumberHash>, Vec<EthcontractEvent<C::Event>>) {
        let (mut blocks_filtered, mut events) = (vec![], vec![]);
        for block in blocks {
            match self.contract.get_events().block_hash(block.1).query().await {
                Ok(e) => {
                    blocks_filtered.push(*block);
                    events.extend(e.into_iter());
                }
                Err(_) => return (blocks_filtered, events),
            }
        }
        (blocks_filtered, events)
    }
}

/// Try to shorten the current_blocks by detecting the reorg from previous event update.
/// If no reorg can be detected (for example, when `handled_blocks` is shorter then the
/// reorg depth) then fallback to full `current_block` as a safe measure.
fn detect_reorg_path<'a>(
    handled_blocks: &[BlockNumberHash],
    current_blocks: &'a [BlockNumberHash],
) -> (&'a [BlockNumberHash], bool) {
    // in most cases, current_blocks = handled_blocks + 1 newest block
    // therefore, is it more efficient to put the handled_blocks in outer loop,
    // so everything finishes in only two iterations.
    for handled_block in handled_blocks.iter().rev() {
        for (i, current_block) in current_blocks.iter().enumerate().rev() {
            if current_block == handled_block {
                // found the same block in both lists, now we know the common ancestor, don't include the ancestor
                let is_reorg = handled_block != handled_blocks.last().unwrap();
                return (&current_blocks[i + 1..], is_reorg);
            }
        }
    }

    // reorg deeper than the EventHandler history (`handled_blocks`), return full list
    let is_reorg = !handled_blocks.is_empty();
    (current_blocks, is_reorg)
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

    #[test]
    fn detect_reorg_path_test_both_empty() {
        let handled_blocks = vec![];
        let current_blocks = vec![];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        assert!(replacement_blocks.is_empty());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_handled_blocks_empty() {
        let handled_blocks = vec![];
        let current_blocks = vec![(1, H256::from_low_u64_be(1))];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        assert_eq!(replacement_blocks, current_blocks);
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_both_same() {
        // if the list are same, we return the common ancestor
        let handled_blocks = vec![(1, H256::from_low_u64_be(1))];
        let current_blocks = vec![(1, H256::from_low_u64_be(1))];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        assert!(replacement_blocks.is_empty());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_common_case() {
        let handled_blocks = vec![(1, H256::from_low_u64_be(1)), (2, H256::from_low_u64_be(2))];
        let current_blocks = vec![
            (1, H256::from_low_u64_be(1)),
            (2, H256::from_low_u64_be(2)),
            (3, H256::from_low_u64_be(3)),
            (4, H256::from_low_u64_be(4)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        assert_eq!(replacement_blocks, current_blocks[2..].to_vec());
        assert!(!is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_reorg_1() {
        let handled_blocks = vec![
            (1, H256::from_low_u64_be(1)),
            (2, H256::from_low_u64_be(2)),
            (3, H256::from_low_u64_be(3)),
        ];
        let current_blocks = vec![
            (1, H256::from_low_u64_be(1)),
            (2, H256::from_low_u64_be(2)),
            (3, H256::from_low_u64_be(4)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        assert_eq!(replacement_blocks, current_blocks[2..].to_vec());
        assert!(is_reorg);
    }

    #[test]
    fn detect_reorg_path_test_reorg_no_common_ancestor() {
        let handled_blocks = vec![(2, H256::from_low_u64_be(20))];
        let current_blocks = vec![
            (1, H256::from_low_u64_be(11)),
            (2, H256::from_low_u64_be(21)),
            (3, H256::from_low_u64_be(31)),
        ];
        let (replacement_blocks, is_reorg) = detect_reorg_path(&handled_blocks, &current_blocks);
        assert_eq!(replacement_blocks, current_blocks);
        assert!(is_reorg);
    }
}
