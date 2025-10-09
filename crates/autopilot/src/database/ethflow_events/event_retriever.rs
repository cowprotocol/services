//! A component that listens exclusively for `OrderRefund` events of the ethflow
//! contract.

use {
    alloy::{
        primitives::Address,
        rpc::types::{Filter, FilterSet},
        sol_types::SolEvent,
    },
    contracts::alloy::CoWSwapEthFlow::CoWSwapEthFlow,
    shared::{ethrpc::Web3, event_handling::AlloyEventRetrieving},
};

pub struct EthFlowRefundRetriever {
    web3: Web3,
    addresses: Vec<Address>,
}

impl EthFlowRefundRetriever {
    pub fn new(web3: Web3, addresses: Vec<Address>) -> Self {
        assert!(
            !addresses.is_empty(),
            "EthFlowRefundRetriever must have at least one address to listen to."
        );
        Self { web3, addresses }
    }
}

impl AlloyEventRetrieving for EthFlowRefundRetriever {
    type Event = CoWSwapEthFlow::CoWSwapEthFlowEvents;

    fn provider(&self) -> &contracts::alloy::Provider {
        &self.web3.alloy
    }

    fn filter(&self) -> alloy::rpc::types::Filter {
        Filter::new()
            .event_signature(FilterSet::from_iter([
                CoWSwapEthFlow::OrderRefund::SIGNATURE_HASH,
            ]))
            .address(self.addresses.clone())
    }
}
