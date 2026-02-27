use {
    crate::event_handling::{AlloyEventRetrieving, EventStoring},
    UniswapV3Pool::UniswapV3Pool::UniswapV3PoolEvents as AlloyUniswapV3PoolEvents,
    alloy::{
        primitives::Address,
        providers::DynProvider,
        rpc::types::{Filter, FilterSet, Log},
        sol_types::SolEvent,
    },
    anyhow::{Context, Result},
    contracts::{
        UniswapV3Pool,
        UniswapV3Pool::UniswapV3Pool::{Burn, Mint, Swap, UniswapV3PoolEvents},
    },
    ethrpc::block_stream::RangeInclusive,
    std::collections::BTreeMap,
};

pub struct UniswapV3PoolEventFetcher(pub DynProvider);

impl AlloyEventRetrieving for UniswapV3PoolEventFetcher {
    type Event = AlloyUniswapV3PoolEvents;

    fn filter(&self) -> Filter {
        // No pool address filter since the generated request might be too large
        // leading to RPC performance issues.
        // More details: <https://github.com/cowprotocol/services/pull/3620>
        let signature_filter = FilterSet::from_iter([
            Swap::SIGNATURE_HASH,
            Burn::SIGNATURE_HASH,
            Mint::SIGNATURE_HASH,
        ]);
        Filter::new()
            .address(vec![])
            .event_signature(signature_filter)
    }

    fn provider(&self) -> &DynProvider {
        &self.0
    }
}

/// Just a helper container to keep track of the event's originating contract
/// address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WithAddress<T>(T, Address);

impl<T> WithAddress<T> {
    pub fn new(inner: T, address: Address) -> Self {
        Self(inner, address)
    }

    pub fn inner(&self) -> &T {
        &self.0
    }

    pub fn address(&self) -> Address {
        self.1
    }
}

#[derive(Default)]
pub struct RecentEventsCache {
    /// (block number, event log index) used as a Key
    events: BTreeMap<(u64, usize), WithAddress<UniswapV3PoolEvents>>,
}

impl RecentEventsCache {
    /// Removes all events up to the specified block, excluding the specified
    /// block.
    pub fn remove_events_older_than_block(&mut self, delete_up_to_block_number: u64) {
        self.events = self.events.split_off(&(delete_up_to_block_number, 0));
    }

    /// Removes all events from the specified block, including specified block.
    fn remove_events_newer_than_block(&mut self, delete_from_block_number: u64) {
        self.events.split_off(&(delete_from_block_number, 0));
    }

    pub fn get_events(
        &self,
        block_range: RangeInclusive<u64>,
    ) -> Vec<WithAddress<UniswapV3PoolEvents>> {
        self.events
            .range((*block_range.start(), 0)..=(*block_range.end(), usize::MAX))
            .map(|(_, event)| event)
            .cloned()
            .collect()
    }
}

#[async_trait::async_trait]
impl EventStoring<(AlloyUniswapV3PoolEvents, Log)> for RecentEventsCache {
    async fn replace_events(
        &mut self,
        events: Vec<(AlloyUniswapV3PoolEvents, Log)>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        self.remove_events_newer_than_block(*range.start());
        self.append_events(events).await
    }

    async fn append_events(&mut self, events: Vec<(AlloyUniswapV3PoolEvents, Log)>) -> Result<()> {
        for (event, log) in events {
            let block_number = log.block_number.context("log block number is empty")?;
            let log_index = log
                .log_index
                .context("log index is empty")?
                .try_into()
                .context("log index too large")?;
            self.events.insert(
                (block_number, log_index),
                WithAddress::new(event, log.address()),
            );
        }
        Ok(())
    }

    async fn last_event_block(&self) -> Result<u64> {
        self.events
            .keys()
            .last()
            .map(|(block_number, _)| block_number)
            .cloned()
            .context("no events")
    }

    async fn persist_last_indexed_block(&mut self, _block: u64) -> Result<()> {
        // storage is only in-memory so we don't need to persist anything here
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::aliases::I24, num::ToPrimitive};

    fn build_event((block_number, log_index): (u64, usize)) -> WithAddress<UniswapV3PoolEvents> {
        WithAddress::new(
            UniswapV3PoolEvents::Swap(build_swap_event(block_number, log_index)),
            Default::default(),
        )
    }

    fn build_swap_event(block_number: u64, tick: usize) -> Swap {
        Swap {
            sender: Default::default(),
            recipient: Default::default(),
            amount0: Default::default(),
            amount1: Default::default(),
            sqrtPriceX96: Default::default(),
            // Encode some values to properly compare the items
            liquidity: block_number.to_u128().unwrap(),
            tick: I24::try_from(tick).unwrap(),
        }
    }

    fn build_alloy_event(
        (block_number, log_index): (u64, usize),
    ) -> (AlloyUniswapV3PoolEvents, Log) {
        let event = AlloyUniswapV3PoolEvents::Swap(build_swap_event(block_number, log_index));
        let log = Log {
            block_number: Some(block_number),
            log_index: Some(log_index.try_into().unwrap()),
            ..Default::default()
        };

        (event, log)
    }

    #[test]
    fn remove_events_older_than_block_test_empty() {
        let mut cache = RecentEventsCache::default();
        cache.remove_events_older_than_block(5);
    }

    #[test]
    fn remove_events_older_than_block_test() {
        let keys = [(1, 0), (1, 1), (2, 0), (2, 1), (3, 0), (3, 1)];
        let events = keys
            .into_iter()
            .map(|key| (key, build_event(key)))
            .collect();

        let mut cache = RecentEventsCache { events };
        cache.remove_events_older_than_block(2);

        assert_eq!(cache.events.keys().cloned().collect::<Vec<_>>(), keys[2..]);
    }

    #[test]
    fn remove_events_newer_than_block_test_empty() {
        let mut cache = RecentEventsCache::default();
        cache.remove_events_newer_than_block(5);
    }

    #[test]
    fn remove_events_newer_than_block_test() {
        let keys = [(1, 0), (1, 1), (2, 0), (2, 1), (3, 0), (3, 1)];
        let events = keys
            .into_iter()
            .map(|key| (key, build_event(key)))
            .collect();

        let mut cache = RecentEventsCache { events };
        cache.remove_events_newer_than_block(2);

        assert_eq!(cache.events.keys().cloned().collect::<Vec<_>>(), keys[..2]);
    }

    #[test]
    fn get_events_test_empty() {
        let cache = RecentEventsCache::default();
        let events = cache.get_events(RangeInclusive::try_new(5u64, 5).unwrap());
        assert!(events.is_empty());
    }

    #[test]
    fn get_events_test() {
        let keys = [
            (1, 0),
            (1, 1),
            (2, 0),
            (2, 1),
            (3, 0),
            (3, 1),
            (4, 0),
            (4, 1),
        ];
        let events = keys
            .into_iter()
            .map(|key| (key, build_event(key)))
            .collect();
        let cache = RecentEventsCache { events };

        // test inside range
        let expected_events = keys[2..=5]
            .iter()
            .map(|key| build_event(*key))
            .collect::<Vec<_>>();
        let events = cache.get_events(RangeInclusive::try_new(2u64, 3).unwrap());
        assert_eq!(events, expected_events);

        // test wide range
        let expected_events = keys[2..=7]
            .iter()
            .map(|key| build_event(*key))
            .collect::<Vec<_>>();
        let events = cache.get_events(RangeInclusive::try_new(2u64, 7).unwrap());
        assert_eq!(events, expected_events);
    }

    #[tokio::test]
    async fn append_events_test() {
        let events = BTreeMap::from([((1, 0), build_event((1, 0))), ((1, 1), build_event((1, 1)))]);
        let mut cache = RecentEventsCache { events };

        let appended_events = vec![
            build_alloy_event((1, 2)),
            build_alloy_event((2, 0)),
            build_alloy_event((2, 1)),
        ];
        cache.append_events(appended_events).await.unwrap();

        let expected_events = BTreeMap::from([
            ((1, 0), build_event((1, 0))),
            ((1, 1), build_event((1, 1))),
            ((1, 2), build_event((1, 2))),
            ((2, 0), build_event((2, 0))),
            ((2, 1), build_event((2, 1))),
        ]);
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
        let keys = [(1, 0), (1, 1), (2, 0), (2, 1)];
        let events = keys
            .into_iter()
            .map(|key| (key, build_event(key)))
            .collect();
        let cache = RecentEventsCache { events };
        let result = cache.last_event_block().await.unwrap();
        assert_eq!(result, 2);
    }
}
