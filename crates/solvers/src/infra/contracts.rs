use crate::domain::eth;

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
}

impl Contracts {
    pub fn for_chain(chain: eth::ChainId) -> Self {
        Self {
            weth: eth::WethAddress(
                contracts::WETH9::raw_contract()
                    .networks
                    .get(chain.network_id())
                    .expect("contract address for all supported chains")
                    .address,
            ),
        }
    }
}
