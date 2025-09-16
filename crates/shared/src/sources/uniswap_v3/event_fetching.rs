use {
    crate::event_handling::{AlloyEventRetrieving, EventStoring},
    UniswapV3Pool::UniswapV3Pool::UniswapV3PoolEvents as AlloyUniswapV3PoolEvents,
    alloy::{
        primitives::Address,
        providers::DynProvider,
        rpc::types::{Filter, Log},
        sol_types::SolEvent,
    },
    anyhow::{Context, Result},
    contracts::alloy::{
        UniswapV3Pool,
        UniswapV3Pool::UniswapV3Pool::{Burn, Mint, Swap},
    },
    ethrpc::block_stream::RangeInclusive,
    maplit::hashset,
    std::collections::BTreeMap,
};

pub struct UniswapV3PoolEventFetcher(pub DynProvider);

impl AlloyEventRetrieving for UniswapV3PoolEventFetcher {
    type Event = AlloyUniswapV3PoolEvents;

    fn filter(&self) -> Filter {
        Filter::new().address(vec![]).event_signature(hashset![
            Swap::SIGNATURE_HASH,
            Burn::SIGNATURE_HASH,
            Mint::SIGNATURE_HASH
        ])
    }

    fn provider(&self) -> &DynProvider {
        &self.0
    }
}

#[derive(Clone)]
pub struct WithAddress<T>(pub T, pub Address);

// AlloyUniswapV3PoolEvents doesn't derive Clone, so we need this wrapper
#[derive(Clone)]
pub enum UniswapV3PoolEvent {
    Swap(WithAddress<Swap>),
    Burn(WithAddress<Burn>),
    Mint(WithAddress<Mint>),
}

impl UniswapV3PoolEvent {
    pub fn address(&self) -> Address {
        match self {
            UniswapV3PoolEvent::Swap(WithAddress(_, address)) => *address,
            UniswapV3PoolEvent::Burn(WithAddress(_, address)) => *address,
            UniswapV3PoolEvent::Mint(WithAddress(_, address)) => *address,
        }
    }
}

impl TryFrom<(&AlloyUniswapV3PoolEvents, &Log)> for UniswapV3PoolEvent {
    type Error = ();

    fn try_from(
        (event, log): (&AlloyUniswapV3PoolEvents, &Log),
    ) -> std::result::Result<Self, Self::Error> {
        match event {
            AlloyUniswapV3PoolEvents::Swap(event) => Ok(UniswapV3PoolEvent::Swap(WithAddress(
                event.clone(),
                log.address(),
            ))),
            AlloyUniswapV3PoolEvents::Burn(event) => Ok(UniswapV3PoolEvent::Burn(WithAddress(
                event.clone(),
                log.address(),
            ))),
            AlloyUniswapV3PoolEvents::Mint(event) => Ok(UniswapV3PoolEvent::Mint(WithAddress(
                event.clone(),
                log.address(),
            ))),
            _ => Err(()),
        }
    }
}

#[derive(Default)]
pub struct RecentEventsCache {
    /// (block number, event log index) used as a Key
    events: BTreeMap<(u64, usize), UniswapV3PoolEvent>,
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

    pub fn get_events(&self, block_range: RangeInclusive<u64>) -> Vec<UniswapV3PoolEvent> {
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
            let Ok(event) = UniswapV3PoolEvent::try_from((&event, &log)) else {
                continue;
            };
            let block_number = log.block_number.context("log block number is empty")?;
            let log_index = log
                .log_index
                .context("log index is empty")?
                .try_into()
                .context("log index too large")?;
            self.events.insert((block_number, log_index), event);
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
    use {
        super::*,
        alloy::primitives::{I256, U160, U256, aliases::I24},
        num::ToPrimitive,
        std::{
            fmt,
            fmt::{Debug, Formatter},
        },
    };

    trait EventKey {
        type Key: Eq + Debug;

        fn key(&self) -> Self::Key;
    }

    impl EventKey for Swap {
        type Key = (Address, Address, U160, I256, I256, u128, I24);

        fn key(&self) -> Self::Key {
            (
                self.sender,
                self.recipient,
                self.sqrtPriceX96,
                self.amount0,
                self.amount1,
                self.liquidity,
                self.tick,
            )
        }
    }

    impl EventKey for Burn {
        type Key = (Address, U256, U256, u128, I24, I24);

        fn key(&self) -> Self::Key {
            (
                self.owner,
                self.amount0,
                self.amount1,
                self.amount,
                self.tickLower,
                self.tickUpper,
            )
        }
    }

    impl EventKey for Mint {
        type Key = (Address, Address, U256, U256, u128, I24, I24);

        fn key(&self) -> Self::Key {
            (
                self.sender,
                self.owner,
                self.amount0,
                self.amount1,
                self.amount,
                self.tickLower,
                self.tickUpper,
            )
        }
    }

    impl PartialEq for UniswapV3PoolEvent {
        fn eq(&self, other: &Self) -> bool {
            use UniswapV3PoolEvent::*;
            match (self, other) {
                (Swap(WithAddress(a, addr_a)), Swap(WithAddress(b, addr_b))) => {
                    addr_a == addr_b && a.key() == b.key()
                }
                (Burn(WithAddress(a, addr_a)), Burn(WithAddress(b, addr_b))) => {
                    addr_a == addr_b && a.key() == b.key()
                }
                (Mint(WithAddress(a, addr_a)), Mint(WithAddress(b, addr_b))) => {
                    addr_a == addr_b && a.key() == b.key()
                }
                _ => false,
            }
        }
    }

    impl Debug for UniswapV3PoolEvent {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
            use UniswapV3PoolEvent::*;
            match self {
                Swap(WithAddress(event, address)) => f
                    .debug_struct("Swap")
                    .field("address", address)
                    .field("sender", &event.sender)
                    .field("recipient", &event.recipient)
                    .field("amount0", &event.amount0)
                    .field("amount1", &event.amount1)
                    .field("sqrtPriceX96", &event.sqrtPriceX96)
                    .field("liquidity", &event.liquidity)
                    .field("tick", &event.tick)
                    .finish(),
                Burn(WithAddress(event, address)) => f
                    .debug_struct("Burn")
                    .field("address", address)
                    .field("owner", &event.owner)
                    .field("amount0", &event.amount0)
                    .field("amount1", &event.amount1)
                    .field("amount", &event.amount)
                    .field("tickLower", &event.tickLower)
                    .field("tickUpper", &event.tickUpper)
                    .finish(),
                Mint(WithAddress(event, address)) => f
                    .debug_struct("Mint")
                    .field("address", address)
                    .field("sender", &event.sender)
                    .field("owner", &event.owner)
                    .field("amount0", &event.amount0)
                    .field("amount1", &event.amount1)
                    .field("amount", &event.amount)
                    .field("tickLower", &event.tickLower)
                    .field("tickUpper", &event.tickUpper)
                    .finish(),
            }
        }
    }

    impl Eq for UniswapV3PoolEvent {}

    fn build_event((block_number, log_index): (u64, usize)) -> UniswapV3PoolEvent {
        UniswapV3PoolEvent::Swap(WithAddress(
            build_swap_event((block_number, log_index)),
            Default::default(),
        ))
    }

    fn build_swap_event((block_number, log_index): (u64, usize)) -> Swap {
        Swap {
            sender: Default::default(),
            recipient: Default::default(),
            amount0: Default::default(),
            amount1: Default::default(),
            sqrtPriceX96: Default::default(),
            // Encode some values to properly compare the items
            liquidity: block_number.to_u128().unwrap(),
            tick: I24::try_from(log_index).unwrap(),
        }
    }

    fn build_alloy_event(
        (block_number, log_index): (u64, usize),
    ) -> (AlloyUniswapV3PoolEvents, Log) {
        let event = AlloyUniswapV3PoolEvents::Swap(build_swap_event((block_number, log_index)));
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
