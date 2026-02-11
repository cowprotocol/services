use {
    alloy::{
        primitives::Address,
        rpc::types::{Filter, FilterSet},
        sol_types::SolEvent,
    },
    contracts::alloy::CoWSwapOnchainOrders,
    shared::{web3::Web3, event_handling::AlloyEventRetrieving},
};

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
    type Event = CoWSwapOnchainOrders::CoWSwapOnchainOrders::CoWSwapOnchainOrdersEvents;

    fn filter(&self) -> alloy::rpc::types::Filter {
        Filter::new()
            .address(self.addresses.clone())
            .event_signature(FilterSet::from_iter([
                CoWSwapOnchainOrders::CoWSwapOnchainOrders::OrderInvalidation::SIGNATURE_HASH,
                CoWSwapOnchainOrders::CoWSwapOnchainOrders::OrderPlacement::SIGNATURE_HASH,
            ]))
    }

    fn provider(&self) -> &alloy::providers::DynProvider {
        &self.web3.provider
    }
}
