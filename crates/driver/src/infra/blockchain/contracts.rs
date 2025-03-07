use {
    crate::{boundary, domain::eth, infra::blockchain::Ethereum},
    chain::Chain,
    contracts::FlashLoanRouter,
    ethcontract::dyns::DynWeb3,
    ethrpc::block_stream::CurrentBlockWatcher,
    thiserror::Error,
    url::Url,
};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    vault_relayer: eth::ContractAddress,
    vault: contracts::BalancerV2Vault,
    weth: contracts::WETH9,

    /// The domain separator for settlement contract used for signing orders.
    settlement_domain_separator: eth::DomainSeparator,
    cow_amm_registry: cow_amm::Registry,

    /// Each lender potentially has different solver wrapper.
    flashloan_wrappers: Vec<contracts::IFlashLoanSolverWrapper>,
    /// Single router that supports multiple flashloans in the
    /// same settlement.
    // TODO: make this non-optional when contracts are deployed
    // everywhere
    flashloan_router: Option<FlashLoanRouter>,
}

#[derive(Debug, Default, Clone)]
pub struct Addresses {
    pub settlement: Option<eth::ContractAddress>,
    pub weth: Option<eth::ContractAddress>,
    pub cow_amms: Vec<CowAmmConfig>,
    pub flashloan_wrappers: Vec<eth::ContractAddress>,
    pub flashloan_router: Option<eth::ContractAddress>,
}

impl Contracts {
    pub(super) async fn new(
        web3: &DynWeb3,
        chain: Chain,
        addresses: Addresses,
        block_stream: CurrentBlockWatcher,
        archive_node_url: Option<&Url>,
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

        let archive_node_web3 = archive_node_url
            .as_ref()
            .map_or(web3.clone(), |url| boundary::buffered_web3_client(url));
        let mut cow_amm_registry = cow_amm::Registry::new(archive_node_web3);
        for config in addresses.cow_amms {
            cow_amm_registry
                .add_listener(config.index_start, config.factory, config.helper)
                .await;
        }
        cow_amm_registry.spawn_maintenance_task(block_stream);

        let flashloan_wrappers = addresses
            .flashloan_wrappers
            .iter()
            .map(|address| {
                contracts::IFlashLoanSolverWrapper::at(
                    web3,
                    address_for(
                        contracts::IFlashLoanSolverWrapper::raw_contract(),
                        Some(*address),
                    ),
                )
            })
            .collect();

        // TODO: use `address_for()` once contracts are deployed
        let flashloan_router = addresses
            .flashloan_router
            .or_else(|| {
                contracts::FlashLoanRouter::raw_contract()
                    .networks
                    .get(&chain.id().to_string())
                    .map(|deployment| eth::ContractAddress(deployment.address))
            })
            .map(|address| contracts::FlashLoanRouter::at(web3, address.0));

        Ok(Self {
            settlement,
            vault_relayer,
            vault,
            weth,
            settlement_domain_separator,
            cow_amm_registry,
            flashloan_wrappers,
            flashloan_router,
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

    pub fn cow_amm_registry(&self) -> &cow_amm::Registry {
        &self.cow_amm_registry
    }

    pub fn flashloan_wrappers(&self) -> &[contracts::IFlashLoanSolverWrapper] {
        &self.flashloan_wrappers
    }

    pub fn flashloan_router(&self) -> Option<&contracts::FlashLoanRouter> {
        self.flashloan_router.as_ref()
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
