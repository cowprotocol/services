use {
    crate::domain::eth,
    chain::Chain,
    contracts::alloy::WETH9,
    ethrpc::alloy::conversions::IntoLegacy,
};

#[derive(Clone, Debug)]
pub struct Contracts {
    pub weth: eth::WethAddress,
}

impl Contracts {
    pub fn for_chain(chain: Chain) -> Self {
        Self {
            weth: eth::WethAddress(
                WETH9::deployment_address(&chain.id())
                    .unwrap_or_else(|| {
                        // For local development chains (Hardhat/Anvil), use the standard deployment
                        // address from the test deployment at 0x5FbDB2315678afecb367f032d93F642f64180aa3
                        if chain.id() == 31337 {
                            "0x5FbDB2315678afecb367f032d93F642f64180aa3".parse().unwrap()
                        } else {
                            panic!("there should be a contract address for all supported chains")
                        }
                    })
                    .into_legacy(),
            ),
        }
    }
}
