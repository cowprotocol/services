use {
    crate::{domain::eth, infra::blockchain::Ethereum},
    ethcontract::dyns::DynWeb3,
    ethrpc::current_block::CurrentBlockStream,
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
    cow_amms: cow_amm::Registry,
}

#[derive(Debug, Default, Clone)]
pub struct Addresses {
    pub settlement: Option<eth::ContractAddress>,
    pub weth: Option<eth::ContractAddress>,
    pub cow_amms: Vec<CowAmmConfig>,
}

impl Contracts {
    pub(super) async fn new(
        web3: &DynWeb3,
        chain: eth::ChainId,
        addresses: Addresses,
        block_stream: CurrentBlockStream,
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

        let cow_amms = cow_amm::Registry::new(web3.clone(), block_stream);
        for config in addresses.cow_amms {
            cow_amms
                .add_listener(config.index_start, config.factory, config.helper)
                .await;
        }

        Ok(Self {
            settlement,
            vault_relayer,
            vault,
            weth,
            settlement_domain_separator,
            cow_amms,
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

    pub fn cow_amms(&self) -> &cow_amm::Registry {
        &self.cow_amms
    }
}

#[derive(Debug, Clone)]
pub struct CowAmmConfig {
    /// Which contract to index for CoW AMM deployment events.
    pub factory: eth::H160,
    /// Which helper contract to use for interfacing with the indexed CoW AMMs.
    pub helper: eth::H160,
    /// At which block indexing should start on the factory.
    pub index_start: u64,
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
