use {
    self::contracts::Contracts,
    crate::{boundary, domain::eth},
    alloy::{
        providers::{Provider, ext::DebugApi},
        rpc::types::{
            TransactionReceipt,
            trace::geth::{GethDebugBuiltInTracerType, GethDebugTracingOptions, GethTrace},
        },
    },
    anyhow::bail,
    chain::Chain,
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    thiserror::Error,
    url::Url,
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
    pub async fn new(url: &url::Url, ethrpc_args: &shared::web3::Arguments) -> Result<Self, Error> {
        let web3 = boundary::web3_client(url, ethrpc_args);
        let chain = Chain::try_from(web3.provider.get_chain_id().await?)
            .map_err(|_| Error::UnsupportedChain)?;

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
        current_block_args: &shared::current_block::Arguments,
    ) -> Self {
        let contracts = Contracts::new(&web3, chain, addresses).await;

        Self {
            current_block: current_block_args
                .stream(url, unbuffered_web3.provider.clone())
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

    pub async fn transaction(&self, hash: eth::TxId) -> Result<eth::Transaction, Error> {
        let (receipt, traces): (Option<TransactionReceipt>, GethTrace) = tokio::try_join!(
            self.web3.provider.get_transaction_receipt(hash.0),
            // Use unbuffered transport for the Debug API since not all providers support
            // batched debug calls.
            self.unbuffered_web3.provider.debug_trace_transaction(
                hash.0,
                GethDebugTracingOptions::new_tracer(GethDebugBuiltInTracerType::CallTracer),
            )
        )?;

        let receipt = receipt.ok_or(Error::TransactionNotFound)?;
        let block_hash =
            receipt
                .block_hash
                .ok_or(Error::IncompleteTransactionData(anyhow::anyhow!(
                    "missing block_hash"
                )))?;
        let block = self
            .web3
            .provider
            .get_block_by_hash(block_hash)
            .await?
            .ok_or(Error::TransactionNotFound)?;

        into_domain(receipt, traces, block.header.timestamp)
            .map_err(Error::IncompleteTransactionData)
    }
}

fn into_domain(
    receipt: TransactionReceipt,
    trace: GethTrace,
    timestamp: u64,
) -> anyhow::Result<eth::Transaction> {
    let trace_calls = match trace {
        GethTrace::CallTracer(call_frame) => call_frame.into(),
        trace => bail!("unsupported trace call {trace:?}"),
    };

    Ok(eth::Transaction {
        hash: receipt.transaction_hash.into(),
        from: receipt.from,
        block: receipt
            .block_number
            .ok_or(anyhow::anyhow!("missing block_number"))?
            .into(),
        gas: receipt.gas_used.into(),
        gas_price: receipt.effective_gas_price.into(),
        timestamp: u32::try_from(timestamp)?,
        trace_calls,
    })
}

impl From<alloy::rpc::types::trace::geth::CallFrame> for eth::CallFrame {
    fn from(value: alloy::rpc::types::trace::geth::CallFrame) -> Self {
        Self {
            from: value.from,
            to: value.to,
            input: value.input,
            calls: value.calls.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("alloy transport error: {0:?}")]
    Alloy(#[from] alloy::transports::TransportError),
    #[error("missing field {0}, node client bug?")]
    IncompleteTransactionData(anyhow::Error),
    #[error("transaction not found")]
    TransactionNotFound,
    #[error("unsupported chain")]
    UnsupportedChain,
}
