use contracts::cowswap_onchain_orders;
use ethcontract::{contract::AllEventsBuilder, transport::DynTransport, H160, H256};
use hex_literal::hex;
use shared::{
    current_block::RangeInclusive,
    ethrpc::Web3,
    event_handling::{EventRetrieving, EventStoring},
};

use crate::database::Postgres;

const ORDER_PLACEMENT_TOPIC: H256 = H256(hex!(
    "cf5f9de2984132265203b5c335b25727702ca77262ff622e136baa7362bf1da9"
));
const ORDER_INVALIDATION_TOPIC: H256 = H256(hex!(
    "b8bad102ac8bbacfef31ff1c906ec6d951c230b4dce750bb0376b812ad35852a"
));
static ALL_VALID_ONCHAIN_ORDER_TOPICS: [H256; 2] =
    [ORDER_PLACEMENT_TOPIC, ORDER_INVALIDATION_TOPIC];

// Note: we use a custom implementation of `EventRetrieving` rather than using the one that is
// automatically derivable from the onchain-order contract. This is because the Rust implementation
// of the onchain-order contract assumes that only events that appear in the ABI can be emitted.
// In this custom implementation, we ignore all events except for those specified by the above
// hardcoded topics (which should correspond to the topics of all avents in the onchain-order
// contract ABI).
pub struct CoWSwapOnchainOrdersContract {
    web3: Web3,
    address: H160,
}

impl CoWSwapOnchainOrdersContract {
    pub fn new(web3: Web3, address: H160) -> Self {
        Self { web3, address }
    }
}

impl EventRetrieving for CoWSwapOnchainOrdersContract {
    type Event = cowswap_onchain_orders::Event;

    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        let mut events = AllEventsBuilder::new(self.web3.clone(), self.address, None);
        // Filter out events that don't belong to the ABI of `OnchainOrdersContract`. This is done
        // because there could be other unrelated events fired by the contract which should be
        // ignored. Also, it makes the request more efficient, since it needs to return less events.
        events.filter = events
            .filter
            .topic0(ALL_VALID_ONCHAIN_ORDER_TOPICS.to_vec().into());
        events
    }
}

const ORDER_REFUND_TOPIC: H256 = H256(hex!(
    // TODO find correct topic
    "b8bad102ac8bbacfef31ff1c906ec6d951c230b4dce750bb0376b812ad35852a"
));

pub struct EthFlowContract {
    web3: Web3,
    address: H160,
}

impl EthFlowContract {
    pub fn new(web3: Web3, address: H160) -> Self {
        Self { web3, address }
    }
}

impl EventRetrieving for EthFlowContract {
    type Event = contracts::cowswap_eth_flow::Event;

    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        let mut events = AllEventsBuilder::new(self.web3.clone(), self.address, None);
        // Filter out events that don't belong to the ABI of `OnchainOrdersContract`. This is done
        // because there could be other unrelated events fired by the contract which should be
        // ignored. Also, it makes the request more efficient, since it needs to return less events.
        events.filter = events.filter.topic0(vec![ORDER_REFUND_TOPIC].into());
        events
    }
}

use anyhow::Result;
type EthFlowEvent = contracts::cowswap_eth_flow::Event;
#[async_trait::async_trait]
impl EventStoring<EthFlowEvent> for Postgres {
    async fn last_event_block(&self) -> Result<u64> {
        todo!()
    }

    async fn append_events(&mut self, events: Vec<ethcontract::Event<EthFlowEvent>>) -> Result<()> {
        todo!()
    }

    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<EthFlowEvent>>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        todo!()
    }
}
