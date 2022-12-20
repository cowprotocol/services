use contracts::cowswap_onchain_orders;
use ethcontract::{contract::AllEventsBuilder, transport::DynTransport, H160, H256};
use hex_literal::hex;
use shared::{ethrpc::Web3, event_handling::EventRetrieving};

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
        // Filter out events that we don't want to listen for in the contract. `Self` is designed to
        // only pick up refunding events. Adding a filter also makes the query more efficient.
        events.filter = events
            .filter
            .topic0(ALL_VALID_ONCHAIN_ORDER_TOPICS.to_vec().into());
        events
    }
}
