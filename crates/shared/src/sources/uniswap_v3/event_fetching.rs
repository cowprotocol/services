use crate::current_block::RangeInclusive;
use crate::event_handling::{EventRetrieving, EventStoring};
use crate::Web3;

use anyhow::{anyhow, Context, Result};
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
        let standard_event = log.topics.get(0).copied().map(|topic| match topic {
            H256 ([12 , 57 , 108 , 217 , 137 , 163 , 159 , 68 , 89 , 181 , 250 , 26 , 237 , 106 , 154 , 141 , 205 , 188 , 69 , 144 , 138 , 207 , 214 , 126 , 2 , 140 , 213 , 104 , 218 , 152 , 152 , 44]) => Ok (UniswapV3Event::Burn (log.clone().decode(UniswapV3Pool::raw_contract().abi.event("Burn").expect("generated event decode"))?)), 
            H256 ([122 , 83 , 8 , 11 , 164 , 20 , 21 , 139 , 231 , 236 , 105 , 185 , 135 , 181 , 251 , 125 , 7 , 222 , 225 , 1 , 254 , 133 , 72 , 143 , 8 , 83 , 174 , 22 , 35 , 157 , 11 , 222]) => Ok (UniswapV3Event::Mint (log.clone().decode(UniswapV3Pool::raw_contract().abi.event("Mint").expect ("generated event decode"))?)), 
            H256 ([196 , 32 , 121 , 249 , 74 , 99 , 80 , 215 , 230 , 35 , 95 , 41 , 23 , 73 , 36 , 249 , 40 , 204 , 42 , 200 , 24 , 235 , 100 , 254 , 216 , 0 , 78 , 17 , 95 , 188 , 202 , 103]) => Ok (UniswapV3Event::Swap (log.clone().decode(UniswapV3Pool::raw_contract().abi.event("Swap").expect ("generated event decode"))?)), 
            _ => Err (ExecutionError::from(Error::Other(std::borrow::Cow::Borrowed("redundant eventy type, skipping...")))),});
        if let Some(Ok(data)) = standard_event {
            return Ok(data);
        }
        Err(ExecutionError::from(Error::InvalidData))
    }
}

pub struct UniswapV3PoolEventFetcher {
    pub web3: Web3,
    pub contracts: Vec<H160>,
}

impl EventRetrieving for UniswapV3PoolEventFetcher {
    type Event = UniswapV3Event;
    fn get_events(&self) -> DynAllEventsBuilder<Self::Event> {
        let mut events = DynAllEventsBuilder::new(self.web3.clone(), H160::default(), None);
        events.filter.address = self.contracts.clone();
        //events.filter.topics =
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
                >= delete_from_block_number
        });
    }

    fn first_event_block(&self) -> Result<u64> {
        Ok(self
            .events
            .first()
            .context("event cache is empty")?
            .meta
            .as_ref()
            .context("event meta is empty")?
            .block_number)
    }

    pub async fn get_events(&self, block_number: u64) -> Result<Vec<Event<UniswapV3Event>>> {
        if block_number < self.first_event_block().context("empty event cache")?
            || block_number > self.last_event_block().await?
        {
            return Err(anyhow!("events cache miss"));
        }

        Ok(self
            .events
            .iter()
            .take_while(|event| {
                event
                    .meta
                    .as_ref()
                    .filter(|event| event.block_number <= block_number)
                    .is_some()
            })
            .cloned()
            .collect())
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
