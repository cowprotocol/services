use {
    chain::Chain,
    configs::simulator::Addresses,
    contracts::alloy::{GPv2Settlement, WETH9},
    ethrpc::Web3,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    pub settlement: GPv2Settlement::Instance,
    pub weth: WETH9::Instance,
}

impl Contracts {
    pub fn new(web3: Web3, chain: Chain, addresses: Addresses) -> Self {
        let settlement = GPv2Settlement::Instance::new(
            addresses
                .settlement
                .or_else(|| GPv2Settlement::deployment_address(&chain.id()))
                .unwrap(),
            web3.provider.clone(),
        );
        let weth = WETH9::Instance::new(
            addresses
                .weth
                .or_else(|| WETH9::deployment_address(&chain.id()))
                .unwrap(),
            web3.provider.clone(),
        );

        Self { settlement, weth }
    }
}
