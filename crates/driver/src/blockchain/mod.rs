use {
    crate::logic::eth,
    ethcontract::{transport::DynTransport, Web3},
    thiserror::Error,
    url::Url,
};

pub mod contracts;

const MAX_BATCH_SIZE: usize = 100;

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("deploy error: {0:?}")]
    Deploy(#[from] ethcontract::errors::DeployError),
}

/// The Ethereum blockchain.
#[derive(Debug)]
pub struct Ethereum {
    web3: Web3<DynTransport>,
    chain_id: eth::ChainId,
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

// TODO This could probably do some caching, I might want to open an issue for
// this?
pub struct Contracts<'a>(&'a Ethereum);

impl Contracts<'_> {
    /// The settlement contract.
    pub async fn settlement(&self) -> Result<eth::Contract, Error> {
        Ok(contracts::GPv2Settlement::deployed(&self.0.web3)
            .await?
            .address()
            .into())
    }

    /// The WETH contract. This should return [`eth::Contract`] in the future.
    /// For now, due to boundary integration reasons, it returns the actual
    /// WETH9 contract.
    pub async fn weth(&self) -> Result<contracts::WETH9, Error> {
        contracts::WETH9::deployed(&self.0.web3)
            .await
            .map_err(Into::into)
    }
}
