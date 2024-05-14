use {super::ChainId, crate::domain, ethcontract::dyns::DynWeb3, primitive_types::H160};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    weth: contracts::WETH9,
    chainalysis_oracle: Option<contracts::ChainalysisOracle>,

    /// The authenticator contract used for allow-listing solvers to settle.
    authenticator: contracts::GPv2AllowListAuthentication,
    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: domain::eth::DomainSeparator,
}

#[derive(Debug, Clone)]
pub struct Addresses {
    pub settlement: Option<H160>,
    pub weth: Option<H160>,
}

impl Contracts {
    pub async fn new(web3: &DynWeb3, chain: &ChainId, addresses: Addresses) -> Self {
        let address_for = |contract: &ethcontract::Contract, address: Option<H160>| {
            address
                .or_else(|| deployment_address(contract, chain))
                .unwrap()
        };

        let settlement = contracts::GPv2Settlement::at(
            web3,
            address_for(
                contracts::GPv2Settlement::raw_contract(),
                addresses.settlement,
            ),
        );

        let weth = contracts::WETH9::at(
            web3,
            address_for(contracts::WETH9::raw_contract(), addresses.weth),
        );

        let chainalysis_oracle = contracts::ChainalysisOracle::deployed(web3).await.ok();

        let settlement_domain_separator = domain::eth::DomainSeparator(
            settlement
                .domain_separator()
                .call()
                .await
                .expect("domain separator")
                .0,
        );

        let authenticator = contracts::GPv2AllowListAuthentication::at(
            web3,
            settlement
                .authenticator()
                .call()
                .await
                .expect("authenticator address"),
        );

        Self {
            settlement,
            weth,
            chainalysis_oracle,
            settlement_domain_separator,
            authenticator,
        }
    }

    pub fn settlement(&self) -> &contracts::GPv2Settlement {
        &self.settlement
    }

    pub fn settlement_domain_separator(&self) -> &domain::eth::DomainSeparator {
        &self.settlement_domain_separator
    }

    pub fn chainalysis_oracle(&self) -> &Option<contracts::ChainalysisOracle> {
        &self.chainalysis_oracle
    }

    pub fn weth(&self) -> &contracts::WETH9 {
        &self.weth
    }

    pub fn authenticator(&self) -> &contracts::GPv2AllowListAuthentication {
        &self.authenticator
    }
}

/// Returns the address of a contract for the specified network, or `None` if
/// there is no known deployment for the contract on that network.
pub fn deployment_address(contract: &ethcontract::Contract, chain: &ChainId) -> Option<H160> {
    Some(contract.networks.get(&chain.to_string())?.address)
}
