//! A component that listens exclusively for `OrderRefund` events of the ethflow contract.
use ethcontract::{contract::AllEventsBuilder, transport::DynTransport, H160, H256};
use hex_literal::hex;
use shared::{ethrpc::Web3, event_handling::EventRetrieving};

const ORDER_REFUND_TOPIC: H256 = H256(hex!(
    "195271068a288191e4b265c641a56b9832919f69e9e7d6c2f31ba40278aeb85a"
));

pub struct EthFlowRefundRetriever {
    web3: Web3,
    address: H160,
}

impl EthFlowRefundRetriever {
    pub fn new(web3: Web3, address: H160) -> Self {
        Self { web3, address }
    }
}

impl EventRetrieving for EthFlowRefundRetriever {
    type Event = contracts::cowswap_eth_flow::Event;

    fn get_events(&self) -> AllEventsBuilder<DynTransport, Self::Event> {
        let mut events = AllEventsBuilder::new(self.web3.clone(), self.address, None);
        // Filter out events that we don't want to listen for in the contract. `Self` is designed to
        // only pick up refunding events. Adding a filter also makes the query more efficient.
        events.filter = events.filter.topic0(vec![ORDER_REFUND_TOPIC].into());
        events
    }
}
