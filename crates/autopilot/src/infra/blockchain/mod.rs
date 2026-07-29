use {
    self::contracts::Contracts,
    crate::{
        boundary,
        domain::{self, blockchain::CallFrame},
    },
    alloy::{
        providers::Provider,
        rpc::types::{
            TransactionReceipt,
            trace::geth::{GethDebugBuiltInTracerType, GethDebugTracingOptions},
        },
    },
    chain::Chain,
    eth_domain_types as eth,
    ethrpc::{Web3, block_stream::CurrentBlockWatcher},
    futures::TryFutureExt as _,
    serde::Deserialize,
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

    pub async fn transaction(
        &self,
        hash: eth::TxId,
    ) -> Result<domain::blockchain::Transaction, Error> {
        let (receipt, traces): (Option<TransactionReceipt>, CallFrame) = tokio::try_join!(
            self.web3
                .provider
                .get_transaction_receipt(hash.0)
                .map_err(Error::Alloy),
            // Use unbuffered transport for the Debug API since not all providers support
            // batched debug calls.
            fetch_debug_trace(&self.unbuffered_web3.provider, hash)
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

/// Fetches the debug traces for the given transaction while bypassing serde's
/// default recursion limit. This is needed to support transactions with
/// exceptionally deep call stacks. (e.g.
/// <https://dashboard.tenderly.co/tx/0x5200aab12fbe4e0aef019748bf0f79266155fbbea00557bf1071fa2859e7eb9b>)
async fn fetch_debug_trace(provider: &impl Provider, hash: eth::TxId) -> Result<CallFrame, Error> {
    let params = serde_json::value::to_raw_value(&(
        hash.0,
        GethDebugTracingOptions::new_tracer(GethDebugBuiltInTracerType::CallTracer),
    ))
    .expect("serialization of known-good types");

    let raw = provider
        .raw_request_dyn("debug_traceTransaction".into(), &params)
        .await?;

    let mut de = serde_json::Deserializer::from_str(raw.get());
    de.disable_recursion_limit();
    // use [`serde_stacker`] to dynamically grow the stack in a vector to avoid
    // blowing the stack size limit when recursing through the object
    let de = serde_stacker::Deserializer::new(&mut de);

    CallFrame::deserialize(de).map_err(Error::DeserializationFailed)
}

fn into_domain(
    receipt: TransactionReceipt,
    trace: CallFrame,
    timestamp: u64,
) -> anyhow::Result<domain::blockchain::Transaction> {
    Ok(domain::blockchain::Transaction {
        hash: receipt.transaction_hash.into(),
        from: receipt.from,
        block: receipt
            .block_number
            .ok_or(anyhow::anyhow!("missing block_number"))?
            .into(),
        gas: receipt.gas_used.into(),
        gas_price: receipt.effective_gas_price.into(),
        timestamp: u32::try_from(timestamp)?,
        trace_calls: trace,
    })
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
    #[error("failed to deserialize tx: {0:?}")]
    DeserializationFailed(#[from] serde_json::Error),
}
