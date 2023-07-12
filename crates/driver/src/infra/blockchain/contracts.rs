use {
    crate::{domain::eth, infra::blockchain::Ethereum},
    ethcontract::dyns::DynWeb3,
};

pub use crate::boundary::contracts::{GPv2Settlement, IUniswapLikeRouter, ERC20, WETH9};

#[derive(Debug, Clone)]
pub struct Contracts {
    settlement: contracts::GPv2Settlement,
    weth: contracts::WETH9,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Addresses {
    pub settlement: Option<eth::ContractAddress>,
    pub weth: Option<eth::ContractAddress>,
}

impl Contracts {
    pub(super) fn new(web3: &DynWeb3, network_id: &eth::NetworkId, addresses: Addresses) -> Self {
        let address = addresses
            .settlement
            .or_else(|| deployment_address(contracts::GPv2Settlement::raw_contract(), network_id))
            .unwrap()
            .into();
        let settlement = contracts::GPv2Settlement::at(web3, address);

        let address = addresses
            .weth
            .or_else(|| deployment_address(contracts::WETH9::raw_contract(), network_id))
            .unwrap()
            .into();
        let weth = contracts::WETH9::at(web3, address);
        Self { settlement, weth }
    }

    pub fn settlement(&self) -> &contracts::GPv2Settlement {
        &self.settlement
    }

    pub fn weth(&self) -> &contracts::WETH9 {
        &self.weth
    }

    pub fn weth_address(&self) -> eth::WethAddress {
        self.weth.address().into()
    }
}

/// Returns the address of a contract for the specified network, or `None` if
/// there is no known deployment for the contract on that network.
pub fn deployment_address(
    contract: &ethcontract::Contract,
    network_id: &eth::NetworkId,
) -> Option<eth::ContractAddress> {
    Some(contract.networks.get(network_id.as_str())?.address.into())
}

/// A trait for initializing contract instances with dynamic addresses.
pub trait ContractAt {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self;
}

impl ContractAt for IUniswapLikeRouter {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self {
        Self::at(&eth.web3, address.0)
    }
}

impl ContractAt for ERC20 {
    fn at(eth: &Ethereum, address: eth::ContractAddress) -> Self {
        ERC20::at(&eth.web3, address.into())
    }
}
