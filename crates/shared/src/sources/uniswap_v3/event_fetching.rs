use std::str::FromStr;

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
                H256(hex!("0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c")) => {
                    Ok(UniswapV3Event::Burn(
                        log.clone().decode(
                            UniswapV3Pool::raw_contract()
                                .abi
                                .event("Burn")
                                .expect("generated event decode"),
                        )?,
                    ))
                }
                H256(hex!("7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde")) => {
                    Ok(UniswapV3Event::Mint(
                        log.clone().decode(
                            UniswapV3Pool::raw_contract()
                                .abi
                                .event("Mint")
                                .expect("generated event decode"),
                        )?,
                    ))
                }
                H256(hex!("c42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67")) => {
                    Ok(UniswapV3Event::Swap(
                        log.clone().decode(
                            UniswapV3Pool::raw_contract()
                                .abi
                                .event("Swap")
                                .expect("generated event decode"),
                        )?,
                    ))
                }
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
        let events_signatures = vec![
            H256::from_str(
                "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67", //swap
            )
            .unwrap(),
            H256::from_str(
                "0x0c396cd989a39f4459b5fa1aed6a9a8dcdbc45908acfd67e028cd568da98982c", //burn
            )
            .unwrap(),
            H256::from_str(
                "0x7a53080ba414158be7ec69b987b5fb7d07dee101fe85488f0853ae16239d0bde", //mint
            )
            .unwrap(),
        ];
        events.filter = events
            .filter
            .address(vec![])
            .topic0(events_signatures.into());
        events
    }
}

#[derive(Debug, Default)]
pub struct RecentEventsCache {
    /// Events are ordered by block number
    events: Vec<Event<UniswapV3Event>>,
}

impl RecentEventsCache {
    /// Removes all events from the specified block.
    pub fn remove_events_newer_than_block(&mut self, delete_from_block_number: u64) {
        self.events.retain(|event| {
            event
                .meta
                .as_ref()
                .expect("events must have metadata")
                .block_number
                < delete_from_block_number
        });
    }

    pub fn get_events(&self, block_range: RangeInclusive<u64>) -> Vec<Event<UniswapV3Event>> {
        self.events
            .iter()
            .filter(|event| {
                event
                    .meta
                    .as_ref()
                    .map(|event| {
                        event.block_number >= *block_range.start()
                            && event.block_number <= *block_range.end()
                    })
                    .unwrap_or_default()
            })
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
        self.events.extend(events);
        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        Ok(self
            .events
            .last()
            .context("event cache is empty")?
            .meta
            .as_ref()
            .context("event meta is empty")?
            .block_number)
    }
}
