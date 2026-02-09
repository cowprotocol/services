use {
    crate::domain,
    alloy::primitives::Address,
    chain::Chain,
    contracts::alloy::{
        ChainalysisOracle,
        GPv2AllowListAuthentication,
        GPv2Settlement,
        HooksTrampoline,
        WETH9,
        support::Balances,
    },
    ethrpc::Web3,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: GPv2Settlement::Instance,
    signatures: contracts::alloy::support::Signatures::Instance,
    weth: WETH9::Instance,
    balances: Balances::Instance,
    chainalysis_oracle: Option<ChainalysisOracle::Instance>,
    trampoline: HooksTrampoline::Instance,

    /// The authenticator contract that decides which solver is allowed to
    /// submit settlements.
    authenticator: GPv2AllowListAuthentication::Instance,
    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: domain::eth::DomainSeparator,
}

#[derive(Debug, Clone)]
pub struct Addresses {
    pub settlement: Option<Address>,
    pub signatures: Option<Address>,
    pub weth: Option<Address>,
    pub balances: Option<Address>,
    pub trampoline: Option<Address>,
}

impl Contracts {
    pub async fn new(web3: &Web3, chain: &Chain, addresses: Addresses) -> Self {
        let settlement = GPv2Settlement::Instance::new(
            addresses
                .settlement
                .or_else(|| GPv2Settlement::deployment_address(&chain.id()))
                .unwrap(),
            web3.provider.clone(),
        );

        let signatures = contracts::alloy::support::Signatures::Instance::new(
            addresses
                .signatures
                .or_else(|| contracts::alloy::support::Signatures::deployment_address(&chain.id()))
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

        let balances = Balances::Instance::new(
            addresses
                .balances
                .or_else(|| Balances::deployment_address(&chain.id()))
                .unwrap(),
            web3.provider.clone(),
        );

        let trampoline = HooksTrampoline::Instance::new(
            addresses
                .trampoline
                .or_else(|| HooksTrampoline::deployment_address(&chain.id()))
                .unwrap(),
            web3.provider.clone(),
        );

        let chainalysis_oracle = ChainalysisOracle::Instance::deployed(&web3.provider)
            .await
            .ok();

        let settlement_domain_separator = domain::eth::DomainSeparator(
            settlement
                .domainSeparator()
                .call()
                .await
                .expect("domain separator")
                .0,
        );

        let authenticator = GPv2AllowListAuthentication::Instance::new(
            settlement
                .authenticator()
                .call()
                .await
                .expect("authenticator address"),
            web3.provider.clone(),
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

    pub fn settlement(&self) -> &GPv2Settlement::Instance {
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

    pub fn weth(&self) -> &WETH9::Instance {
        &self.weth
    }

    /// Wrapped version of the native token (e.g. WETH for Ethereum, WXDAI for
    /// Gnosis Chain)
    pub fn wrapped_native_token(&self) -> domain::eth::WrappedNativeToken {
        (*self.weth.address()).into()
    }

    pub fn authenticator(&self) -> &GPv2AllowListAuthentication::Instance {
        &self.authenticator
    }
}
