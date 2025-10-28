use {
    crate::{domain::eth, infra::blockchain::Ethereum},
    chain::Chain,
    contracts::alloy::{
        BalancerV2Vault,
        FlashLoanRouter,
        GPv2Settlement,
        WETH9,
        support::Balances,
    },
    ethrpc::{
        Web3,
        alloy::conversions::{IntoAlloy, IntoLegacy},
    },
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
    ) -> Result<Self, Error> {
        let settlement = GPv2Settlement::Instance::new(
            addresses
                .settlement
                .map(|addr| addr.0.into_alloy())
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
                .map(|addr| addr.0.into_alloy())
                .or_else(|| Balances::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );
        let signatures = contracts::alloy::support::Signatures::Instance::new(
            addresses
                .signatures
                .map(|addr| addr.0.into_alloy())
                .or_else(|| contracts::alloy::support::Signatures::deployment_address(&chain.id()))
                .unwrap(),
            web3.alloy.clone(),
        );

        let weth = WETH9::Instance::new(
            addresses
                .weth
                .map(|addr| addr.0.into_alloy())
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
            .or_else(|| {
                FlashLoanRouter::deployment_address(&chain.id()).map(|deployment_address| {
                    eth::ContractAddress(deployment_address.into_legacy())
                })
            })
            .map(|address| {
                FlashLoanRouter::Instance::new(address.0.into_alloy(), web3.alloy.clone())
            });

        Ok(Self {
            settlement,
            vault_relayer: vault_relayer.into_legacy().into(),
            vault,
            signatures,
            weth,
            settlement_domain_separator,
            flashloan_router,
            balance_helper,
            cow_amm_helper_by_factory: addresses.cow_amm_helper_by_factory,
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
        self.weth.address().into_legacy().into()
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

    pub fn cow_amm_helper_by_factory(
        &self,
    ) -> &HashMap<eth::ContractAddress, eth::ContractAddress> {
        &self.cow_amm_helper_by_factory
    }
}

/// Returns the address of a contract for the specified network, or `None` if
/// there is no known deployment for the contract on that network.
pub fn deployment_address(
    contract: &ethcontract::Contract,
    chain: Chain,
) -> Option<eth::ContractAddress> {
    Some(
        contract
            .networks
            .get(&chain.id().to_string())?
            .address
            .into(),
    )
}

/// A trait for initializing contract instances with dynamic addresses.
pub trait ContractAt {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self;
}

impl ContractAt for contracts::ERC20 {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self {
        Self::at(&eth.web3, address.into())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("method error: {0:?}")]
    Rpc(#[from] alloy::contract::Error),
}
