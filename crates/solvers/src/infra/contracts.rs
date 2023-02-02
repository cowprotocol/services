use {crate::domain::eth, ethcontract::H160};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Addresses {
    pub weth: Option<eth::WethAddress>,
}

impl Contracts {
    pub fn new(chain: eth::ChainId, addresses: Addresses) -> Self {
        Self {
            weth: addresses.weth.unwrap_or_else(|| {
                eth::WethAddress(address_for_chain(contracts::WETH9::raw_contract(), chain))
            }),
        }
    }

    pub fn weth(&self) -> &eth::WethAddress {
        &self.weth
    }
}

fn address_for_chain(contract: &ethcontract::Contract, chain: eth::ChainId) -> H160 {
    contract
        .networks
        .get(chain.network_id())
        .expect("contract address for all supported chains")
        .address
}
