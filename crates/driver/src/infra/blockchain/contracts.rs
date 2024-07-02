use {
    crate::{domain::eth, infra::blockchain::Ethereum},
    ethcontract::{dyns::DynWeb3, errors::DeployError},
    thiserror::Error,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    vault_relayer: eth::ContractAddress,
    vault: contracts::BalancerV2Vault,
    weth: contracts::WETH9,

    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: eth::DomainSeparator,
    cow_amm_legacy_helper: Option<contracts::CowAmmLegacyHelper>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Addresses {
    pub settlement: Option<eth::ContractAddress>,
    pub weth: Option<eth::ContractAddress>,
}

impl Contracts {
    pub(super) async fn new(
        web3: &DynWeb3,
        chain: eth::ChainId,
        addresses: Addresses,
    ) -> Result<Self, Error> {
        let address_for = |contract: &ethcontract::Contract,
                           address: Option<eth::ContractAddress>| {
            address
                .or_else(|| deployment_address(contract, chain))
                .unwrap()
                .0
        };

        let settlement = contracts::GPv2Settlement::at(
            web3,
            address_for(
                contracts::GPv2Settlement::raw_contract(),
                addresses.settlement,
            ),
        );
        let vault_relayer = settlement.methods().vault_relayer().call().await?.into();
        let vault =
            contracts::BalancerV2Vault::at(web3, settlement.methods().vault().call().await?);

        let weth = contracts::WETH9::at(
            web3,
            address_for(contracts::WETH9::raw_contract(), addresses.weth),
        );

        let settlement_domain_separator = eth::DomainSeparator(
            settlement
                .domain_separator()
                .call()
                .await
                .expect("domain separator")
                .0,
        );

        let cow_amm_legacy_helper = match contracts::CowAmmLegacyHelper::deployed(web3).await {
            Err(DeployError::NotFound(_)) => None,
            Err(err) => panic!("failed to find deployed contract: {:?}", err),
            Ok(contract) => Some(contract),
        };

        Ok(Self {
            settlement,
            vault_relayer,
            vault,
            weth,
            settlement_domain_separator,
            cow_amm_legacy_helper,
        })
    }

    pub fn settlement(&self) -> &contracts::GPv2Settlement {
        &self.settlement
    }

    pub fn vault_relayer(&self) -> eth::ContractAddress {
        self.vault_relayer
    }

    pub fn vault(&self) -> &contracts::BalancerV2Vault {
        &self.vault
    }

    pub fn weth(&self) -> &contracts::WETH9 {
        &self.weth
    }

    pub fn weth_address(&self) -> eth::WethAddress {
        self.weth.address().into()
    }

    pub fn settlement_domain_separator(&self) -> &eth::DomainSeparator {
        &self.settlement_domain_separator
    }

    pub fn cow_amm_legacy_helper(&self) -> Option<&contracts::CowAmmLegacyHelper> {
        self.cow_amm_legacy_helper.as_ref()
    }
}

/// Returns the address of a contract for the specified network, or `None` if
/// there is no known deployment for the contract on that network.
pub fn deployment_address(
    contract: &ethcontract::Contract,
    network_id: eth::ChainId,
) -> Option<eth::ContractAddress> {
    Some(
        contract
            .networks
            .get(&network_id.to_string())?
            .address
            .into(),
    )
}

/// A trait for initializing contract instances with dynamic addresses.
pub trait ContractAt {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self;
}

impl ContractAt for contracts::IUniswapLikeRouter {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self {
        Self::at(&eth.web3, address.0)
    }
}

impl ContractAt for contracts::ERC20 {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self {
        Self::at(&eth.web3, address.into())
    }
}

impl ContractAt for contracts::support::Balances {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self {
        Self::at(&eth.web3, address.into())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
}
