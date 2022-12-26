use {crate::logic::eth, thiserror::Error, web3::Web3};

pub mod contracts;

/// The Ethereum blockchain.
#[derive(Debug)]
pub struct Ethereum {
    web3: Web3<web3::transports::Http>,
    chain_id: eth::ChainId,
    network_id: eth::NetworkId,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API hosted at the given
    /// URL.
    pub async fn eth_rpc(url: &str) -> Result<Self, web3::Error> {
        // TODO Enable batching, reuse ethrpc? Put it in the boundary module?
        // I feel like what we have in shared::ethrpc could be simplified if we use
        // web3::transports::batch or something, but I haven't looked deep into it
        let web3 = Web3::new(web3::transports::Http::new(url)?);
        let chain_id = web3.eth().chain_id().await?.into();
        let network_id = web3.net().version().await?.into();
        Ok(Self {
            web3,
            chain_id,
            network_id,
        })
    }

    pub fn chain_id(&self) -> eth::ChainId {
        self.chain_id
    }

    pub fn network_id(&self) -> &eth::NetworkId {
        &self.network_id
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
            .get(self.0.network_id().to_str())
            .unwrap()
            .address;
        contracts::GPv2Settlement::at(&self.0.web3, address)
    }

    /// The WETH contract.
    pub fn weth(&self) -> contracts::WETH9 {
        let address = contracts::WETH9::raw_contract()
            .networks
            .get(self.0.network_id().to_str())
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
