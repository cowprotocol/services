use {
    contracts::cow_amm_legacy_helper::Event as CowAmmEvent,
    ethcontract::{contract::AllEventsBuilder, dyns::DynTransport, Address, H256},
    ethrpc::Web3,
    shared::event_handling::EventRetrieving,
};

const AMM_DEPLOYED_TOPIC: H256 = H256(hex_literal::hex!(
    "0d03834d0d86c7f57e877af40e26f176dc31bd637535d4ba153d1ac9de88a7ea"
));

pub(crate) struct Factory {
    pub(crate) web3: Web3,
    pub(crate) address: Address,
}

impl EventRetrieving for Factory {
    type Event = CowAmmEvent;

    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        let mut events = AllEventsBuilder::new(self.web3.clone(), self.address, None);
        events.filter = events.filter.topic0(Some(AMM_DEPLOYED_TOPIC).into());
        events
    }
}
