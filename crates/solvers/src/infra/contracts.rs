use {
    crate::domain::eth,
    alloy::primitives::Address,
    chain::Chain,
    contracts::{GPv2AllowListAuthentication, GPv2Settlement, WETH9},
};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
    pub settlement: Address,
    pub authenticator: Address,
}

impl Contracts {
    pub fn for_chain(chain: Chain) -> Self {
        let chain_id = chain.id();
        Self {
            weth: eth::WethAddress(
                WETH9::deployment_address(&chain_id)
                    .expect("there should be a contract address for all supported chains"),
            ),
            settlement: GPv2Settlement::deployment_address(&chain_id)
                .expect("there should be a contract address for all supported chains"),
            authenticator: GPv2AllowListAuthentication::deployment_address(&chain_id)
                .expect("there should be a contract address for all supported chains"),
        }
    }

    pub fn for_chain_id(chain_id: eth::ChainId) -> Self {
        let id = chain_id as u64;
        Self {
            weth: eth::WethAddress(
                WETH9::deployment_address(&id)
                    .expect("there should be a contract address for all supported chains"),
            ),
            settlement: GPv2Settlement::deployment_address(&id)
                .expect("there should be a contract address for all supported chains"),
            authenticator: GPv2AllowListAuthentication::deployment_address(&id)
                .expect("there should be a contract address for all supported chains"),
        }
    }
}
