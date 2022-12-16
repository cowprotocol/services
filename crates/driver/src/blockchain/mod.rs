use {
    crate::logic::eth,
    ethcontract::{transport::DynTransport, Web3},
    thiserror::Error,
    url::Url,
};

pub mod contracts;

/// The Ethereum blockchain.
#[derive(Debug)]
pub struct Ethereum {
    web3: Web3<DynTransport>,
    chain_id: eth::ChainId,
    network: eth::NetworkName,
}

impl Ethereum {
    /// Access the Ethereum blockchain through the RPC API of a node.
    pub fn node(_url: Url) -> Self {
        todo!()
    }

    pub fn domain_separator(&self, verifying_contract: eth::Contract) -> eth::DomainSeparator {
        eth::DomainSeparator::new(self.chain_id, verifying_contract)
    }

    pub fn contracts(&self) -> Contracts<'_> {
        Contracts(self)
    }

    /// Fetch the ERC20 allowance for the spender. See the allowance method in
    /// EIP-20.
    ///
    /// https://eips.ethereum.org/EIPS/eip-20#methods
    pub async fn allowance(
        &self,
        owner: eth::Address,
        spender: eth::allowance::Spender,
    ) -> Result<eth::allowance::Existing, Error> {
        let amount = contracts::ERC20::at(&self.web3, spender.token.0)
            .allowance(owner.0, spender.address.0)
            .call()
            .await?;
        Ok(eth::Allowance { spender, amount }.into())
    }
}

pub struct Contracts<'a>(&'a Ethereum);

impl Contracts<'_> {
    /// The settlement contract.
    pub fn settlement(&self) -> contracts::GPv2Settlement {
        let address = contracts::GPv2Settlement::raw_contract()
            .networks
            .get(&self.0.network.0)
            .unwrap()
            .address;
        contracts::GPv2Settlement::at(&self.0.web3, address)
    }

    /// The WETH contract.
    pub fn weth(&self) -> contracts::WETH9 {
        let address = contracts::WETH9::raw_contract()
            .networks
            .get(&self.0.network.0)
            .unwrap()
            .address;
        contracts::WETH9::at(&self.0.web3, address)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
}
