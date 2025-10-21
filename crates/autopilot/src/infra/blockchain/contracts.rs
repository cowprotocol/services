use {
    crate::domain,
    chain::Chain,
    contracts::alloy::{ChainalysisOracle, HooksTrampoline, InstanceExt, support::Balances},
    ethrpc::{Web3, alloy::conversions::IntoAlloy},
    primitive_types::H160,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    signatures: contracts::alloy::support::Signatures::Instance,
    weth: contracts::WETH9,
    balances: Balances::Instance,
    chainalysis_oracle: Option<ChainalysisOracle::Instance>,
    trampoline: HooksTrampoline::Instance,

    /// The authenticator contract that decides which solver is allowed to
    /// submit settlements.
    authenticator: contracts::GPv2AllowListAuthentication,
    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: domain::eth::DomainSeparator,
}

#[derive(Debug, Clone)]
pub struct Addresses {
    pub settlement: Option<H160>,
    pub signatures: Option<H160>,
    pub weth: Option<H160>,
    pub balances: Option<H160>,
    pub trampoline: Option<H160>,
}

impl Contracts {
    pub async fn new(web3: &Web3, chain: &Chain, addresses: Addresses) -> Self {
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

        let signatures = contracts::alloy::support::Signatures::Instance::new(
            addresses
                .signatures
                .map(IntoAlloy::into_alloy)
                .or_else(|| contracts::alloy::support::Signatures::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );

        let weth = contracts::WETH9::at(
            web3,
            address_for(contracts::WETH9::raw_contract(), addresses.weth),
        );

        let balances = Balances::Instance::new(
            addresses
                .balances
                .map(IntoAlloy::into_alloy)
                .or_else(|| Balances::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );

        let trampoline = HooksTrampoline::Instance::new(
            addresses
                .trampoline
                .map(IntoAlloy::into_alloy)
                .or_else(|| HooksTrampoline::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );

        let chainalysis_oracle = ChainalysisOracle::Instance::deployed(&web3.alloy)
            .await
            .ok();

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
            signatures,
            weth,
            balances,
            chainalysis_oracle,
            settlement_domain_separator,
            authenticator,
            trampoline,
        }
    }

    pub fn settlement(&self) -> &contracts::GPv2Settlement {
        &self.settlement
    }

    pub fn balances(&self) -> &Balances::Instance {
        &self.balances
    }

    pub fn signatures(&self) -> &contracts::alloy::support::Signatures::Instance {
        &self.signatures
    }

    pub fn trampoline(&self) -> &HooksTrampoline::Instance {
        &self.trampoline
    }

    pub fn settlement_domain_separator(&self) -> &domain::eth::DomainSeparator {
        &self.settlement_domain_separator
    }

    pub fn chainalysis_oracle(&self) -> &Option<ChainalysisOracle::Instance> {
        &self.chainalysis_oracle
    }

    pub fn weth(&self) -> &contracts::WETH9 {
        &self.weth
    }

    /// Wrapped version of the native token (e.g. WETH for Ethereum, WXDAI for
    /// Gnosis Chain)
    pub fn wrapped_native_token(&self) -> domain::eth::WrappedNativeToken {
        self.weth.address().into()
    }

    pub fn authenticator(&self) -> &contracts::GPv2AllowListAuthentication {
        &self.authenticator
    }
}

/// Returns the address of a contract for the specified chain, or `None` if
/// there is no known deployment for the contract on that chain.
pub fn deployment_address(contract: &ethcontract::Contract, chain: &Chain) -> Option<H160> {
    contract
        .networks
        .get(&chain.id().to_string())
        .map(|network| network.address)
}
