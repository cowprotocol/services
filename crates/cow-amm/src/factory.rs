use {
    alloy_primitives::Address,
    alloy_provider::DynProvider,
    alloy_rpc_types::{Filter, FilterSet},
    alloy_sol_types::SolEvent,
    contracts::alloy::cow_amm::CowAmmLegacyHelper::CowAmmLegacyHelper::{
        self,
        CowAmmLegacyHelperEvents as CowAmmEvent,
    },
    ethrpc::AlloyProvider,
    event_indexing::event_handler::AlloyEventRetrieving,
};

pub(crate) struct Factory {
    pub(crate) provider: AlloyProvider,
    pub(crate) address: Address,
}

impl AlloyEventRetrieving for Factory {
    type Event = CowAmmEvent;

    fn filter(&self) -> Filter {
        Filter::new()
            .address(self.address)
            .event_signature(FilterSet::from_iter([
                CowAmmLegacyHelper::COWAMMPoolCreated::SIGNATURE_HASH,
            ]))
    }

    fn provider(&self) -> &DynProvider {
        &self.provider
    }
}
