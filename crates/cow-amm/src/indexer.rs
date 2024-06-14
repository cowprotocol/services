use {
    crate::events::Event,
    ethcontract::{common::DeploymentInformation, dyns::DynWeb3, Address, Bytes},
    ethrpc::current_block::RangeInclusive,
    shared::event_handling::EventStoring,
    std::{
        collections::{BTreeMap, HashMap},
        sync::Arc,
    },
    tokio::sync::RwLock,
};

pub type CowAmmRegistry = HashMap<Address, CowAmm>;
/// Registry with the format: (block number, (log index, event))
pub type EventsRegistry = BTreeMap<u64, BTreeMap<usize, Event>>;

/// CoW AMM indexer which stores events in-memory.
#[derive(Clone)]
pub struct Indexer {
    cow_amms: Arc<RwLock<CowAmmRegistry>>,
    events_registry: Arc<RwLock<EventsRegistry>>,
    cow_amm_constant_product_factory_first_block: u64,
    cow_amm_first_block: u64,
}

impl Indexer {
    pub async fn new(web3: &DynWeb3) -> Self {
        let cow_amm_constant_product_factory =
            contracts::CowAmmConstantProductFactory::deployed(web3)
                .await
                .expect("Failed to find deployed CowAmmConstantProductFactory");
        let cow_amm_constant_product_factory_first_block = match cow_amm_constant_product_factory
            .deployment_information()
            .expect("Failed to get deployment information")
        {
            DeploymentInformation::BlockNumber(block) => block,
            _ => panic!("Expected block number"),
        };
        let cow_amm = contracts::CowAmm::deployed(web3)
            .await
            .expect("Failed to find deployed CowAmm");
        let cow_amm_first_block = match cow_amm
            .deployment_information()
            .expect("Failed to get deployment information")
        {
            DeploymentInformation::BlockNumber(block) => block,
            _ => panic!("Expected block number"),
        };
        Self {
            cow_amms: Arc::new(RwLock::new(HashMap::new())),
            events_registry: Arc::new(RwLock::new(BTreeMap::new())),
            cow_amm_constant_product_factory_first_block,
            cow_amm_first_block,
        }
    }

    /// Returns all CoW AMMs that are currently enabled (i.e. able to trade).
    pub async fn enabled_cow_amms(&self) -> Vec<impl crate::CowAmm> {
        let cow_amms = self.cow_amms.read().await;
        cow_amms
            .values()
            .filter(|cow_amm| cow_amm.enabled)
            .cloned()
            .collect::<Vec<_>>()
    }
}

#[async_trait::async_trait]
impl EventStoring<contracts::cow_amm_constant_product_factory::Event> for Indexer {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        // Context to drop the write lock before calling `reapply_events()`
        {
            let range = *range.start()..=*range.end();
            let events_registry = &mut *self.events_registry.write().await;

            // Remove the CowAmmConstantProductFactoryEvent events in the given range
            for key in range {
                if let Some(event_map) = events_registry.get_mut(&key) {
                    event_map.retain(|_, event| {
                        !matches!(event, Event::CowAmmConstantProductFactoryEvent(_))
                    });
                    // If the BTreeMap<usize, Event> becomes empty, remove the u64 key as well
                    if event_map.is_empty() {
                        events_registry.remove(&key);
                    }
                }
            }
        }

        // Revert the CoW AMM status after removing the events in the specified
        // range. Since we cannot know what is the CoW AMM status before an
        // event is reverted, in order to guarantee the CoW AMM status is
        // correct, we need to reapply all the events and substitute
        // the current CoW AMM in memory after the reapplication.
        // It returns the new list of CoW AMMs after all the events in the
        // registry have been applied
        // Context to drop the write lock before calling `reapply_events()`
        {
            let mut cow_amms = HashMap::new();
            for event in &events {
                let event = Event::from(event.clone());
                event.apply_event(&mut cow_amms).await?;
            }
            *self.cow_amms.write().await = cow_amms;
        }

        // Apply the new events
        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm_constant_product_factory::Event>>,
    ) -> anyhow::Result<()> {
        let cow_amms = &mut *self.cow_amms.write().await;
        let events_registry = &mut *self.events_registry.write().await;
        for event in events {
            let meta = event
                .meta
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?;
            let block_number = meta.block_number;
            let log_index = meta.log_index;

            let event = Event::from(event);
            event.apply_event(cow_amms).await?;

            events_registry
                .entry(block_number)
                .or_default()
                .insert(log_index, event);
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let latest_product_factory_event = async {
            for (&key, events) in self.events_registry.read().await.iter().rev() {
                if events
                    .values()
                    .any(|event| matches!(event, Event::CowAmmConstantProductFactoryEvent(_)))
                {
                    return Some(key);
                }
            }
            None
        };
        Ok(latest_product_factory_event
            .await
            .unwrap_or(self.cow_amm_constant_product_factory_first_block))
    }
}

#[async_trait::async_trait]
impl EventStoring<contracts::cow_amm::Event> for Indexer {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm::Event>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        // Context to drop the write lock before calling `reapply_events()`
        {
            let range = *range.start()..=*range.end();
            let events_registry = &mut *self.events_registry.write().await;

            // Remove the CowAmmConstantProductFactoryEvent events in the given range
            for key in range {
                if let Some(event_map) = events_registry.get_mut(&key) {
                    event_map.retain(|_, event| !matches!(event, Event::CowAmmEvent(_)));
                    // If the BTreeMap<usize, Event> becomes empty, remove the u64 key as well
                    if event_map.is_empty() {
                        events_registry.remove(&key);
                    }
                }
            }
        }

        // Revert the CoW AMM status after removing the events in the specified
        // range. Since we cannot know what is the CoW AMM status before an
        // event is reverted, in order to guarantee the CoW AMM status is
        // correct, we need to reapply all the events and substitute
        // the current CoW AMM in memory after the reapplication.
        // It returns the new list of CoW AMMs after all the events in the
        // registry have been applied
        // Context to drop the write lock before calling `reapply_events()`
        {
            let mut cow_amms = HashMap::new();
            for event in &events {
                let event = Event::from(event.clone());
                event.apply_event(&mut cow_amms).await?;
            }
            *self.cow_amms.write().await = cow_amms;
        }

        // Apply the new events
        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::cow_amm::Event>>,
    ) -> anyhow::Result<()> {
        let cow_amms = &mut *self.cow_amms.write().await;
        let events_registry = &mut *self.events_registry.write().await;
        for event in events {
            let meta = event
                .meta
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Event missing meta"))?;
            let block_number = meta.block_number;
            let log_index = meta.log_index;

            let event = Event::from(event);
            event.apply_event(cow_amms).await?;

            events_registry
                .entry(block_number)
                .or_default()
                .insert(log_index, event);
        }
        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let latest_product_factory_event = async {
            for (&key, events) in self.events_registry.read().await.iter().rev() {
                if events
                    .values()
                    .any(|event| matches!(event, Event::CowAmmEvent(_)))
                {
                    return Some(key);
                }
            }
            None
        };
        Ok(latest_product_factory_event
            .await
            .unwrap_or(self.cow_amm_first_block))
    }
}

#[derive(Clone)]
pub struct CowAmm {
    address: Address,
    tradeable_pairs: Vec<Address>,
    // This is a placeholder for the actual CoW AMM arbitrary bytes obtained from tradingEnabled
    // (`TradingParams`).
    bytes: Bytes<[u8; 32]>,
    enabled: bool,
}

impl CowAmm {
    pub fn new(address: Address, tradeable_pairs: &[Address]) -> Self {
        Self {
            address,
            tradeable_pairs: tradeable_pairs.to_vec(),
            bytes: Bytes::default(),
            enabled: false,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn set_bytes(&mut self, bytes: Bytes<[u8; 32]>) {
        self.bytes = bytes;
    }
}

impl crate::CowAmm for CowAmm {
    fn address(&self) -> &Address {
        &self.address
    }

    fn traded_tokens(&self) -> &[Address] {
        self.tradeable_pairs.as_slice()
    }
}
