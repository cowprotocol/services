use {
    self::contracts::Contracts,
    crate::{boundary, domain::eth},
    chain::Chain,
    ethcontract::H256,
    ethrpc::{Web3, block_stream::CurrentBlockWatcher, extensions::DebugNamespace},
    futures::TryFutureExt,
    primitive_types::U256,
    std::time::Duration,
    thiserror::Error,
    url::Url,
    web3::types::{Block, TransactionReceipt},
};

pub mod contracts;

/// An Ethereum RPC connection.
pub struct Rpc {
    web3: Web3,
    chain: Chain,
    url: Url,
}

impl Rpc {
    /// Instantiate an RPC client to an Ethereum (or Ethereum-compatible) node
    /// at the specifed URL.
    pub async fn new(
        url: &url::Url,
        ethrpc_args: &shared::ethrpc::Arguments,
    ) -> Result<Self, Error> {
        let web3 = boundary::web3_client(url, ethrpc_args);
        let chain =
            Chain::try_from(web3.eth().chain_id().await?).map_err(|_| Error::UnsupportedChain)?;

        Ok(Self {
            web3,
            chain,
            url: url.clone(),
        })
    }

    /// Returns the chain for the RPC connection.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Returns a reference to the underlying web3 client.
    pub fn web3(&self) -> &Web3 {
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
    web3: Web3,
    unbuffered_web3: Web3,
    chain: Chain,
    current_block: CurrentBlockWatcher,
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
        web3: Web3,
        unbuffered_web3: Web3,
        chain: &Chain,
        url: Url,
        addresses: contracts::Addresses,
        poll_interval: Duration,
    ) -> Self {
        let contracts = Contracts::new(&web3, chain, addresses).await;

        Self {
            current_block: ethrpc::block_stream::current_block_stream(url, poll_interval)
                .await
                .expect("couldn't initialize current block stream"),
            web3,
            unbuffered_web3,
            chain: *chain,
            contracts,
        }
    }

    pub fn chain(&self) -> &Chain {
        &self.chain
    }

    /// Returns a stream that monitors the block chain to inform about the
    /// current and new blocks.
    pub fn current_block(&self) -> &CurrentBlockWatcher {
        &self.current_block
    }

    pub fn contracts(&self) -> &Contracts {
        &self.contracts
    }

    // We fetch the receipt and the block in 1 future to enable
    // fetching the block while the other joint futures in `transaction()`
    // are still executing.
    async fn get_receipt_and_block(
        &self,
        hash: eth::TxId,
    ) -> Result<(TransactionReceipt, Block<H256>), Error> {
        let receipt = self
            .web3
            .eth()
            .transaction_receipt(hash.0)
            .await?
            .ok_or(Error::TransactionNotFound)?;
        let block_hash =
            receipt
                .block_hash
                .ok_or(Error::IncompleteTransactionData(anyhow::anyhow!(
                    "missing block_hash"
                )))?;
        let block = self.web3.eth().block(block_hash.into()).await?.ok_or(
            Error::IncompleteTransactionData(anyhow::anyhow!("block not found")),
        )?;
        Ok((receipt, block))
    }

    pub async fn transaction(&self, hash: eth::TxId) -> Result<eth::Transaction, Error> {
        let (transaction, (receipt, block), traces) = tokio::try_join!(
            self.web3
                .eth()
                .transaction(hash.0.into())
                .map_err(Error::Web3),
            self.get_receipt_and_block(hash),
            // Use unbuffered transport for the Debug API since not all providers support
            // batched debug calls.
            self.unbuffered_web3
                .debug()
                .transaction(hash.0)
                .map_err(Error::Web3),
        )?;
        let transaction = transaction.ok_or(Error::TransactionNotFound)?;
        into_domain(transaction, receipt, traces, block.timestamp)
            .map_err(Error::IncompleteTransactionData)
    }
}

fn into_domain(
    transaction: web3::types::Transaction,
    receipt: web3::types::TransactionReceipt,
    trace_calls: ethrpc::extensions::CallFrame,
    timestamp: U256,
) -> anyhow::Result<eth::Transaction> {
    Ok(eth::Transaction {
        hash: transaction.hash.into(),
        from: transaction
            .from
            .ok_or(anyhow::anyhow!("missing from"))?
            .into(),
        block: receipt
            .block_number
            .ok_or(anyhow::anyhow!("missing block_number"))?
            .0[0]
            .into(),
        gas: receipt
            .gas_used
            .ok_or(anyhow::anyhow!("missing gas_used"))?
            .into(),
        gas_price: receipt
            .effective_gas_price
            .ok_or(anyhow::anyhow!("missing effective_gas_price"))?
            .into(),
        timestamp: timestamp.as_u32(),
        trace_calls: trace_calls.into(),
    })
}

impl From<ethrpc::extensions::CallFrame> for eth::CallFrame {
    fn from(frame: ethrpc::extensions::CallFrame) -> Self {
        eth::CallFrame {
            from: frame.from.into(),
            to: frame.to.map(Into::into),
            input: frame.input.0.into(),
            calls: frame.calls.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("web3 error: {0:?}")]
    Web3(#[from] web3::error::Error),
    #[error("missing field {0}, node client bug?")]
    IncompleteTransactionData(anyhow::Error),
    #[error("transaction not found")]
    TransactionNotFound,
    #[error("unsupported chain")]
    UnsupportedChain,
}
