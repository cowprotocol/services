use {
    alloy::{
        primitives::{Address, B256, b256},
        rpc::types::{Filter, FilterSet},
    },
    shared::{ethrpc::Web3, event_handling::AlloyEventRetrieving},
};

const ORDER_PLACEMENT_TOPIC: B256 =
    b256!("cf5f9de2984132265203b5c335b25727702ca77262ff622e136baa7362bf1da9");
const ORDER_INVALIDATION_TOPIC: B256 =
    b256!("b8bad102ac8bbacfef31ff1c906ec6d951c230b4dce750bb0376b812ad35852a");
static ALL_VALID_ONCHAIN_ORDER_TOPICS: [B256; 2] =
    [ORDER_PLACEMENT_TOPIC, ORDER_INVALIDATION_TOPIC];

// Note: we use a custom implementation of `EventRetrieving` rather than using
// the one that is automatically derivable from the onchain-order contract. This
// is because the Rust implementation of the onchain-order contract assumes that
// only events that appear in the ABI can be emitted. In this custom
// implementation, we ignore all events except for those specified by the above
// hardcoded topics (which should correspond to the topics of all avents in the
// onchain-order contract ABI).
pub struct CoWSwapOnchainOrdersContract {
    web3: Web3,
    addresses: Vec<Address>,
}

impl CoWSwapOnchainOrdersContract {
    pub fn new(web3: Web3, addresses: Vec<Address>) -> Self {
        assert!(
            !addresses.is_empty(),
            "CoWSwapOnchainOrdersContract must have at least one address to listen to."
        );
        Self { web3, addresses }
    }
}

impl AlloyEventRetrieving for CoWSwapOnchainOrdersContract {
    type Event =
        contracts::alloy::CoWSwapOnchainOrders::CoWSwapOnchainOrders::CoWSwapOnchainOrdersEvents;

    fn filter(&self) -> alloy::rpc::types::Filter {
        Filter::new()
            .address(self.addresses.clone())
            .event_signature(FilterSet::from_iter(ALL_VALID_ONCHAIN_ORDER_TOPICS))
    }

    fn provider(&self) -> &contracts::alloy::Provider {
        &self.web3.alloy
    }
}
