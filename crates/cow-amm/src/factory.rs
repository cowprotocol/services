use {
    alloy::{
        primitives::Address,
        providers::DynProvider,
        rpc::types::{Filter, FilterSet},
        sol_types::SolEvent,
    },
    contracts::alloy::cow_amm::CowAmmLegacyHelper::{
        CowAmmLegacyHelper,
        CowAmmLegacyHelper::CowAmmLegacyHelperEvents as CowAmmEvent,
    },
    ethrpc::Web3,
    shared::event_handling::AlloyEventRetrieving,
};

pub(crate) struct Factory {
    pub(crate) web3: Web3,
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
        &self.web3.provider
    }
}
