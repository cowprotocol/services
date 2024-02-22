use {super::ChainId, crate::boundary, ethcontract::dyns::DynWeb3, primitive_types::H160};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    weth: contracts::WETH9,

    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: boundary::DomainSeparator,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Addresses {
    pub settlement: Option<H160>,
    pub weth: Option<H160>,
}

impl Contracts {
    pub async fn new(web3: &DynWeb3, network: &ChainId, addresses: Addresses) -> Self {
        let address_for = |contract: &ethcontract::Contract, address: Option<H160>| {
            address
                .or_else(|| deployment_address(contract, network))
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

        let settlement_domain_separator = boundary::DomainSeparator(
            settlement
                .domain_separator()
                .call()
                .await
                .expect("domain separator")
                .0,
        );

        Self {
            settlement,
            weth,
            settlement_domain_separator,
        }
    }

    pub fn settlement(&self) -> &contracts::GPv2Settlement {
        &self.settlement
    }

    pub fn settlement_domain_separator(&self) -> &model::DomainSeparator {
        &self.settlement_domain_separator
    }

    pub fn weth(&self) -> &contracts::WETH9 {
        &self.weth
    }
}

/// Returns the address of a contract for the specified network, or `None` if
/// there is no known deployment for the contract on that network.
pub fn deployment_address(contract: &ethcontract::Contract, network: &ChainId) -> Option<H160> {
    Some(contract.networks.get(&network.to_string())?.address)
}
