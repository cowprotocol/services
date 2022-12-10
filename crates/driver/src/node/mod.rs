use {
    crate::logic::eth,
    ethcontract::{transport::DynTransport, Web3},
    thiserror::Error,
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

/// The Ethereum node.
#[derive(Debug)]
pub struct EthNode(Web3<DynTransport>);

impl EthNode {
    /// The address of our settlement contract.
    pub async fn settlement_contract(&self) -> Result<eth::Address, Error> {
        Ok(contracts::GPv2Settlement::deployed(&self.0)
            .await?
            .address()
            .into())
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
        let amount = contracts::ERC20::at(&self.0, spender.token.0)
            .allowance(owner.0, spender.address.0)
            .call()
            .await?;
        Ok(eth::Allowance { spender, amount }.into())
    }
}
