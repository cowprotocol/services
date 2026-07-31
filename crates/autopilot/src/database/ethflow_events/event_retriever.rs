//! A component that listens exclusively for `OrderRefund` events of the ethflow
//! contract.

use {
    alloy::{
        primitives::Address,
        rpc::types::{Filter, FilterSet},
        sol_types::SolEvent,
    },
    contracts::CoWSwapEthFlow::CoWSwapEthFlow,
    ethrpc::AlloyProvider,
    event_indexing::event_handler::AlloyEventRetrieving,
};

pub struct EthFlowRefundRetriever {
    provider: AlloyProvider,
    addresses: Vec<Address>,
}

impl EthFlowRefundRetriever {
    pub fn new(provider: AlloyProvider, addresses: Vec<Address>) -> Self {
        assert!(
            !addresses.is_empty(),
            "EthFlowRefundRetriever must have at least one address to listen to."
        );
        Self {
            provider,
            addresses,
        }
    }
}

impl AlloyEventRetrieving for EthFlowRefundRetriever {
    type Event = CoWSwapEthFlow::CoWSwapEthFlowEvents;

    fn provider(&self) -> &alloy::providers::DynProvider {
        &self.provider
    }

    fn filter(&self) -> alloy::rpc::types::Filter {
        Filter::new()
            .event_signature(FilterSet::from_iter([
                CoWSwapEthFlow::OrderRefund::SIGNATURE_HASH,
            ]))
            .address(self.addresses.clone())
    }
}
