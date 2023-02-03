use crate::domain::eth;

#[derive(Clone, Debug)]
pub struct Contracts {
    weth: eth::WethAddress,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Addresses {
    pub weth: Option<eth::WethAddress>,
}

impl Contracts {
    pub fn new(chain: eth::ChainId, addresses: Addresses) -> Self {
        Self {
            weth: addresses.weth.unwrap_or_else(|| {
                eth::WethAddress(
                    contracts::WETH9::raw_contract()
                        .networks
                        .get(chain.network_id())
                        .expect("contract address for all supported chains")
                        .address,
                )
            }),
        }
    }

    pub fn weth(&self) -> &eth::WethAddress {
        &self.weth
    }
}
