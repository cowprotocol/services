use {crate::domain::eth, network::Network};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
    pub settlement: eth::ContractAddress,
    authenticator: eth::ContractAddress,
    pub balancer_vault: eth::ContractAddress,
}

impl Contracts {
    pub fn for_chain(chain: Network) -> Self {
        let a = |contract: &contracts::ethcontract::Contract| {
            eth::ContractAddress(
                contract
                    .networks
                    .get(&chain.chain_id().to_string())
                    .expect("contract address for all supported chains")
                    .address,
            )
        };
        Self {
            weth: eth::WethAddress(a(contracts::WETH9::raw_contract()).0),
            settlement: a(contracts::GPv2Settlement::raw_contract()),
            authenticator: a(contracts::GPv2AllowListAuthentication::raw_contract()),
            balancer_vault: a(contracts::BalancerV2Vault::raw_contract()),
        }
    }
}
