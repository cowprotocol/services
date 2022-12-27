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
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API hosted at the given
    /// URL.
    pub fn eth_rpc(_url: Url) -> Self {
        todo!()
    }

    pub fn chain_id(&self) -> eth::ChainId {
        self.chain_id
    }

    /// Onchain smart contract bindings.
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

    /// Check if a smart contract is deployed to the given address.
    pub async fn is_contract(&self, address: eth::Address) -> Result<bool, Error> {
        let code = self.web3.eth().code(address.into(), None).await?;
        Ok(!code.0.is_empty())
    }
}

pub struct Contracts<'a>(&'a Ethereum);

impl Contracts<'_> {
    /// The settlement contract.
    pub fn settlement(&self) -> contracts::GPv2Settlement {
        let address = contracts::GPv2Settlement::raw_contract()
            .networks
            .get(self.0.chain_id().network_id())
            .unwrap()
            .address;
        contracts::GPv2Settlement::at(&self.0.web3, address)
    }

    /// The WETH contract.
    pub fn weth(&self) -> contracts::WETH9 {
        let address = contracts::WETH9::raw_contract()
            .networks
            .get(self.0.chain_id().network_id())
            .unwrap()
            .address;
        contracts::WETH9::at(&self.0.web3, address)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("method error: {0:?}")]
    Method(#[from] ethcontract::errors::MethodError),
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
}
