use {crate::domain::eth, chain::Chain, contracts::alloy::WETH9};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
}

impl Contracts {
    pub fn for_chain(chain: Chain) -> Self {
        Self {
            weth: eth::WethAddress(
                WETH9::deployment_address(&chain.id())
                    .expect("there should be a contract address for all supported chains"),
            ),
        }
    }
}
