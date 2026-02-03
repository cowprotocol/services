use {
    crate::domain::eth,
    chain::Chain,
    contracts::alloy::{
        BalancerV2Vault,
        FlashLoanRouter,
        GPv2Settlement,
        WETH9,
        support::Balances,
    },
    ethrpc::Web3,
    std::collections::HashMap,
    thiserror::Error,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: GPv2Settlement::Instance,
    vault_relayer: eth::ContractAddress,
    vault: BalancerV2Vault::Instance,
    signatures: contracts::alloy::support::Signatures::Instance,
    weth: WETH9::Instance,

    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: eth::DomainSeparator,

    /// Single router that supports multiple flashloans in the
    /// same settlement.
    // TODO: make this non-optional when contracts are deployed
    // everywhere
    flashloan_router: Option<FlashLoanRouter::Instance>,
    balance_helper: Balances::Instance,
    /// Mapping from CoW AMM factory address to the corresponding CoW AMM
    /// helper.
    cow_amm_helper_by_factory: HashMap<eth::ContractAddress, eth::ContractAddress>,
    web3: Web3,
}

#[derive(Debug, Default, Clone)]
pub struct Addresses {
    pub settlement: Option<eth::ContractAddress>,
    pub signatures: Option<eth::ContractAddress>,
    pub weth: Option<eth::ContractAddress>,
    pub balances: Option<eth::ContractAddress>,
    pub cow_amm_helper_by_factory: HashMap<eth::ContractAddress, eth::ContractAddress>,
    pub flashloan_router: Option<eth::ContractAddress>,
}

impl Contracts {
    pub(super) async fn new(
        web3: &Web3,
        chain: Chain,
        addresses: Addresses,
    ) -> Result<Self, alloy::contract::Error> {
        let settlement = GPv2Settlement::Instance::new(
            addresses
                .settlement
                .map(Into::into)
                .or_else(|| GPv2Settlement::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );
        let vault_relayer = settlement.vaultRelayer().call().await?;
        let vault =
            BalancerV2Vault::Instance::new(settlement.vault().call().await?, web3.alloy.clone());
        let balance_helper = Balances::Instance::new(
            addresses
                .balances
                .map(Into::into)
                .or_else(|| Balances::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );
        let signatures = contracts::alloy::support::Signatures::Instance::new(
            addresses
                .signatures
                .map(Into::into)
                .or_else(|| contracts::alloy::support::Signatures::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );

        let weth = WETH9::Instance::new(
            addresses
                .weth
                .map(Into::into)
                .or_else(|| WETH9::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );

        let settlement_domain_separator = eth::DomainSeparator(
            settlement
                .domainSeparator()
                .call()
                .await
                .expect("domain separator")
                .0,
        );

        // TODO: use `address_for()` once contracts are deployed
        let flashloan_router = addresses
            .flashloan_router
            .or_else(|| FlashLoanRouter::deployment_address(&chain.id()).map(eth::ContractAddress))
            .map(|address| FlashLoanRouter::Instance::new(address.0, web3.alloy.clone()));

        Ok(Self {
            settlement,
            vault_relayer: vault_relayer.into(),
            vault,
            signatures,
            weth,
            settlement_domain_separator,
            flashloan_router,
            balance_helper,
            cow_amm_helper_by_factory: addresses.cow_amm_helper_by_factory,
            web3: web3.clone(),
        })
    }

    pub fn settlement(&self) -> &GPv2Settlement::Instance {
        &self.settlement
    }

    pub fn signatures(&self) -> &contracts::alloy::support::Signatures::Instance {
        &self.signatures
    }

    pub fn vault_relayer(&self) -> eth::ContractAddress {
        self.vault_relayer
    }

    pub fn vault(&self) -> &BalancerV2Vault::Instance {
        &self.vault
    }

    pub fn weth(&self) -> &WETH9::Instance {
        &self.weth
    }

    pub fn weth_address(&self) -> eth::WethAddress {
        (*self.weth.address()).into()
    }

    pub fn settlement_domain_separator(&self) -> &eth::DomainSeparator {
        &self.settlement_domain_separator
    }

    pub fn flashloan_router(&self) -> Option<&FlashLoanRouter::Instance> {
        self.flashloan_router.as_ref()
    }

    pub fn balance_helper(&self) -> &Balances::Instance {
        &self.balance_helper
    }

    pub fn web3(&self) -> &Web3 {
        &self.web3
    }

    pub fn cow_amm_helper_by_factory(
        &self,
    ) -> &HashMap<eth::ContractAddress, eth::ContractAddress> {
        &self.cow_amm_helper_by_factory
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Rpc(#[from] alloy::contract::Error),
}
