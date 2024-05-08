use {
    self::contracts::Contracts,
    crate::boundary,
    ethcontract::dyns::DynWeb3,
    ethrpc::current_block::CurrentBlockStream,
    primitive_types::{H256, U256},
    thiserror::Error,
    url::Url,
};

pub mod contracts;

/// Chain ID as defined by EIP-155.
///
/// https://eips.ethereum.org/EIPS/eip-155
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChainId(pub U256);

impl std::fmt::Display for ChainId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<U256> for ChainId {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

/// An Ethereum RPC connection.
pub struct Rpc {
    web3: DynWeb3,
    chain: ChainId,
    url: Url,
}

impl Rpc {
    /// Instantiate an RPC client to an Ethereum (or Ethereum-compatible) node
    /// at the specifed URL.
    pub async fn new(url: &url::Url) -> Result<Self, Error> {
        let web3 = boundary::buffered_web3_client(url);
        let chain = web3.eth().chain_id().await?.into();

        Ok(Self {
            web3,
            chain,
            url: url.clone(),
        })
    }

    /// Returns the chain id for the RPC connection.
    pub fn chain(&self) -> ChainId {
        self.chain
    }

    /// Returns a reference to the underlying web3 client.
    pub fn web3(&self) -> &DynWeb3 {
        &self.web3
    }
}

/// The Ethereum blockchain.
#[derive(Clone)]
pub struct Ethereum {
    web3: DynWeb3,
    chain: ChainId,
    current_block: CurrentBlockStream,
    contracts: Contracts,
}

impl Ethereum {
    /// Access the Ethereum blockchain through an RPC API.
    ///
    /// # Panics
    ///
    /// Since this type is essential for the program this method will panic on
    /// any initialization error.
    pub async fn new(rpc: Rpc, addresses: contracts::Addresses) -> Self {
        let Rpc { web3, chain, url } = rpc;
        let contracts = Contracts::new(&web3, &chain, addresses).await;

        Self {
            current_block: ethrpc::current_block::current_block_stream(url)
                .await
                .expect("couldn't initialize current block stream"),
            web3,
            chain,
            contracts,
        }
    }

    pub fn network(&self) -> &ChainId {
        &self.chain
    }

    /// Returns a stream that monitors the block chain to inform about the
    /// current and new blocks.
    pub fn current_block(&self) -> &CurrentBlockStream {
        &self.current_block
    }

    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }

    pub async fn transaction(&self, hash: H256) -> Result<Option<web3::types::Transaction>, Error> {
        self.web3
            .eth()
            .transaction(hash.into())
            .await
            .map_err(Into::into)
    }

    pub async fn transaction_receipt(
        &self,
        hash: H256,
    ) -> Result<Option<web3::types::TransactionReceipt>, Error> {
        self.web3
            .eth()
            .transaction_receipt(hash)
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
}
