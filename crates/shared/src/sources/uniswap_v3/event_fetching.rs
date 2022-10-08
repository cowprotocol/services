use std::collections::BTreeMap;

use crate::current_block::RangeInclusive;
use crate::event_handling::{EventRetrieving, EventStoring};
use crate::Web3;
use hex_literal::hex;

use anyhow::{Context, Result};
use contracts::{
    uniswap_v3_pool::event_data::{Burn, Mint, Swap},
    UniswapV3Pool,
};
use ethcontract::{
    common::abi::Error, contract::ParseLog, dyns::DynAllEventsBuilder, errors::ExecutionError,
    Event, RawLog, H160, H256,
};

const SWAP_TOPIC: [u8; 32] =
    hex!("c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67");
const BURN_TOPIC: [u8; 32] =
    hex!("0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c");
const MINT_TOPIC: [u8; 32] =
    hex!("7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UniswapV3Event {
    Burn(Burn),
    Mint(Mint),
    Swap(Swap),
}

impl ParseLog for UniswapV3Event {
    fn parse_log(log: RawLog) -> Result<Self, ExecutionError> {
        let standard_event: Option<Result<UniswapV3Event, ExecutionError>> =
            log.topics.get(0).copied().map(|topic| match topic {
                H256(BURN_TOPIC) => Ok(UniswapV3Event::Burn(
                    log.clone().decode(
                        UniswapV3Pool::raw_contract()
                            .abi
                            .event("Burn")
                            .expect("generated event decode"),
                    )?,
                )),
                H256(MINT_TOPIC) => Ok(UniswapV3Event::Mint(
                    log.clone().decode(
                        UniswapV3Pool::raw_contract()
                            .abi
                            .event("Mint")
                            .expect("generated event decode"),
                    )?,
                )),
                H256(SWAP_TOPIC) => Ok(UniswapV3Event::Swap(
                    log.clone().decode(
                        UniswapV3Pool::raw_contract()
                            .abi
                            .event("Swap")
                            .expect("generated event decode"),
                    )?,
                )),
                _ => Err(ExecutionError::from(Error::InvalidData)),
            });
        if let Some(Ok(data)) = standard_event {
            return Ok(data);
        }
        Err(ExecutionError::from(Error::InvalidData))
    }
}

pub struct UniswapV3PoolEventFetcher(pub Web3);

impl EventRetrieving for UniswapV3PoolEventFetcher {
    type Event = UniswapV3Event;
    fn get_events(&self) -> DynAllEventsBuilder<Self::Event> {
        let mut events = DynAllEventsBuilder::new(self.0.clone(), H160::default(), None);
        let events_signatures = vec![H256(SWAP_TOPIC), H256(BURN_TOPIC), H256(MINT_TOPIC)];
        events.filter = events
            .filter
            .address(vec![])
            .topic0(events_signatures.into());
        events
    }
}

#[derive(Debug, Default)]
pub struct RecentEventsCache {
    /// Block number used as a Key
    events: BTreeMap<u64, Vec<Event<UniswapV3Event>>>,
}

impl RecentEventsCache {
    /// Removes all events from the specified block.
    fn remove_events_newer_than_block(&mut self, delete_from_block_number: u64) {
        self.events
            .retain(|&block_number, _| block_number < delete_from_block_number);
    }

    pub fn get_events(&self, block_range: RangeInclusive<u64>) -> Vec<Event<UniswapV3Event>> {
        self.events
            .range(block_range.start()..=block_range.end())
            .flat_map(|(_, events)| events)
            .cloned()
            .collect()
    }
}

#[async_trait::async_trait]
impl EventStoring<UniswapV3Event> for RecentEventsCache {
    async fn replace_events(
        &mut self,
        events: Vec<Event<UniswapV3Event>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        self.remove_events_newer_than_block(*range.start());
        self.append_events(events).await
    }

    async fn append_events(&mut self, events: Vec<Event<UniswapV3Event>>) -> Result<()> {
        for event in events {
            self.events
                .entry(
                    event
                        .meta
                        .as_ref()
                        .context("event meta is empty")?
                        .block_number,
                )
                .or_default()
                .push(event)
        }
        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        self.events.keys().last().cloned().context("no events")
    }
}

#[cfg(test)]
mod tests {
    use ethcontract::EventMetadata;

    use super::*;

    fn build_event(block_number: u64) -> Event<UniswapV3Event> {
        Event {
            data: UniswapV3Event::Swap(Swap::default()),
            meta: Some(EventMetadata {
                block_number,
                ..Default::default()
            }),
        }
    }

    #[test]
    fn remove_events_newer_than_block_test_empty() {
        let mut cache = RecentEventsCache::default();
        cache.remove_events_newer_than_block(5);
    }

    #[test]
    fn remove_events_newer_than_block_test() {
        let events = BTreeMap::from([
            (1u64, vec![build_event(1)]),
            (2, vec![build_event(2)]),
            (3, vec![build_event(3)]),
        ]);
        let mut cache = RecentEventsCache { events };
        cache.remove_events_newer_than_block(2);

        assert_eq!(cache.events.keys().cloned().collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn get_events_test_empty() {
        let cache = RecentEventsCache::default();
        let events = cache.get_events(RangeInclusive::try_new(5u64, 5).unwrap());
        assert!(events.is_empty());
    }

    #[test]
    fn get_events_test() {
        let events = BTreeMap::from([
            (1u64, vec![build_event(1), build_event(1)]),
            (2, vec![build_event(2), build_event(2)]),
            (3, vec![build_event(3), build_event(3)]),
            (4, vec![build_event(4), build_event(4)]),
        ]);
        let cache = RecentEventsCache { events };

        // test inside range
        let expected_events = vec![
            build_event(2),
            build_event(2),
            build_event(3),
            build_event(3),
        ];
        let events = cache.get_events(RangeInclusive::try_new(2u64, 3).unwrap());
        assert_eq!(events, expected_events);

        // test wide range
        let expected_events = vec![
            build_event(2),
            build_event(2),
            build_event(3),
            build_event(3),
            build_event(4),
            build_event(4),
        ];
        let events = cache.get_events(RangeInclusive::try_new(2u64, 7).unwrap());
        assert_eq!(events, expected_events);
    }

    #[tokio::test]
    async fn append_events_test() {
        let events = BTreeMap::from([(1u64, vec![build_event(1), build_event(1)])]);
        let mut cache = RecentEventsCache { events };

        let appended_events = vec![build_event(1), build_event(2), build_event(2)];
        let expected_events = BTreeMap::from([
            (1u64, vec![build_event(1), build_event(1), build_event(1)]),
            (2, vec![build_event(2), build_event(2)]),
        ]);
        cache.append_events(appended_events).await.unwrap();
        assert_eq!(cache.events, expected_events);
    }

    #[tokio::test]
    async fn last_event_block_test_empty() {
        let cache = RecentEventsCache::default();
        let result = cache.last_event_block().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn last_event_block_test() {
        let events = BTreeMap::from([
            (1u64, vec![build_event(1), build_event(1)]),
            (2, vec![build_event(2), build_event(2)]),
        ]);
        let cache = RecentEventsCache { events };
        let result = cache.last_event_block().await.unwrap();
        assert_eq!(result, 2);
    }
}
