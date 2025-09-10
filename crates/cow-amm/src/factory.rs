use {
    contracts::cow_amm_legacy_helper::Event as CowAmmEvent,
    ethcontract::{
        Address,
        H256,
        contract::AllEventsBuilder,
        dyns::DynAllEventsBuilder,
        futures::Stream,
    },
    ethrpc::{Web3, block_stream::RangeInclusive},
    shared::event_handling::{EthcontractEventQueryBuilder, EventRetrieving},
    std::pin::Pin,
};

const AMM_DEPLOYED_TOPIC: H256 = H256(hex_literal::hex!(
    "0d03834d0d86c7f57e877af40e26f176dc31bd637535d4ba153d1ac9de88a7ea"
));

pub(crate) struct Factory {
    pub(crate) web3: Web3,
    pub(crate) address: Address,
}

impl EthcontractEventQueryBuilder for Factory {
    type Event = CowAmmEvent;

    fn get_events(&self) -> DynAllEventsBuilder<CowAmmEvent> {
        let mut events = AllEventsBuilder::new(self.web3.legacy.clone(), self.address, None);
        events.filter = events.filter.topic0(Some(AMM_DEPLOYED_TOPIC).into());
        events
    }
}

#[async_trait::async_trait]
impl EventRetrieving for Factory {
    type Event = ethcontract::Event<CowAmmEvent>;

    async fn get_events_by_block_hash(
        &self,
        block_hash: H256,
    ) -> anyhow::Result<Vec<ethcontract::Event<CowAmmEvent>>> {
        self.get_events_by_block_hash_default(block_hash).await
    }

    async fn get_events_by_block_range(
        &self,
        block_range: &RangeInclusive<u64>,
    ) -> anyhow::Result<
        Pin<Box<dyn Stream<Item = anyhow::Result<ethcontract::Event<CowAmmEvent>>> + Send>>,
    > {
        self.get_events_by_block_range_default(block_range).await
    }

    fn address(&self) -> Vec<ethcontract::Address> {
        self.address_default()
    }
}
