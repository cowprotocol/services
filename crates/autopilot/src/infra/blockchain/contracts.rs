use {crate::domain, chain::Chain, ethcontract::dyns::DynWeb3, primitive_types::H160};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    signatures: contracts::support::Signatures,
    weth: contracts::WETH9,
    balances: contracts::support::Balances,
    chainalysis_oracle: Option<contracts::ChainalysisOracle>,
    trampoline: contracts::HooksTrampoline,

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
    pub async fn new(web3: &DynWeb3, chain: &Chain, addresses: Addresses) -> Self {
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

        let signatures = contracts::support::Signatures::at(
            web3,
            address_for(
                contracts::support::Signatures::raw_contract(),
                addresses.signatures,
            ),
        );

        let weth = contracts::WETH9::at(
            web3,
            address_for(contracts::WETH9::raw_contract(), addresses.weth),
        );

        let balances = contracts::support::Balances::at(
            web3,
            address_for(
                contracts::support::Balances::raw_contract(),
                addresses.balances,
            ),
        );

        let trampoline = contracts::HooksTrampoline::at(
            web3,
            address_for(
                contracts::HooksTrampoline::raw_contract(),
                addresses.trampoline,
            ),
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

    pub fn balances(&self) -> &contracts::support::Balances {
        &self.balances
    }

    pub fn signatures(&self) -> &contracts::support::Signatures {
        &self.signatures
    }

    pub fn trampoline(&self) -> &contracts::HooksTrampoline {
        &self.trampoline
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
