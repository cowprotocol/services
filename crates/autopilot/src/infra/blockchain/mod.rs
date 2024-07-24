use {
    self::contracts::Contracts,
    crate::{
        boundary,
        domain::{self, eth},
    },
    ethcontract::dyns::DynWeb3,
    ethrpc::current_block::CurrentBlockStream,
    primitive_types::U256,
    std::time::Duration,
    thiserror::Error,
    url::Url,
};

pub mod authenticator;
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

    /// Returns a reference to the underlying RPC URL.
    pub fn url(&self) -> &Url {
        &self.url
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
    pub async fn new(
        web3: DynWeb3,
        chain: ChainId,
        url: Url,
        addresses: contracts::Addresses,
        poll_interval: Duration,
    ) -> Self {
        let contracts = Contracts::new(&web3, &chain, addresses).await;

        Self {
            current_block: ethrpc::current_block::current_block_stream(url, poll_interval)
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

    pub async fn transaction(
        &self,
        hash: eth::TxId,
    ) -> Result<domain::settlement::Transaction, Error> {
        let (transaction, receipt) = tokio::try_join!(
            self.web3.eth().transaction(hash.0.into()),
            self.web3.eth().transaction_receipt(hash.0)
        )?;
        let transaction = transaction.ok_or(Error::TransactionNotFound)?;
        let receipt = receipt.ok_or(Error::TransactionNotFound)?;
        into_domain(transaction, receipt).map_err(Error::IncompleteTransactionData)
    }
}

fn into_domain(
    transaction: web3::types::Transaction,
    receipt: web3::types::TransactionReceipt,
) -> anyhow::Result<domain::settlement::Transaction> {
    Ok(domain::settlement::Transaction {
        hash: transaction.hash.into(),
        solver: transaction
            .from
            .ok_or(anyhow::anyhow!("missing from"))?
            .into(),
        input: transaction.input.0.into(),
        block: receipt
            .block_number
            .ok_or(anyhow::anyhow!("missing block_number"))?
            .0[0]
            .into(),
        gas: receipt
            .gas_used
            .ok_or(anyhow::anyhow!("missing gas_used"))?
            .into(),
        effective_gas_price: receipt
            .effective_gas_price
            .ok_or(anyhow::anyhow!("missing effective_gas_price"))?
            .into(),
    })
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("missing field {0}, node client bug?")]
    IncompleteTransactionData(anyhow::Error),
    #[error("transaction not found")]
    TransactionNotFound,
}
