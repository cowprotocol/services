use {
    self::contracts::Contracts,
    crate::{boundary, domain},
    alloy::{
        providers::Provider,
        rpc::types::{
            TransactionReceipt,
            trace::geth::{GethDebugBuiltInTracerType, GethDebugTracingOptions, GethTrace},
        },
    },
    anyhow::bail,
    chain::Chain,
    eth_domain_types as eth,
    ethrpc::{AlloyProvider, block_stream::CurrentBlockWatcher},
    futures::TryFutureExt as _,
    serde::Deserialize,
    thiserror::Error,
    url::Url,
};

pub mod contracts;

/// An Ethereum RPC connection.
pub struct Rpc {
    provider: AlloyProvider,
    chain: Chain,
    url: Url,
}

impl Rpc {
    /// Instantiate an RPC client to an Ethereum (or Ethereum-compatible) node
    /// at the specifed URL.
    pub async fn new(url: &url::Url, ethrpc_args: &shared::web3::Arguments) -> Result<Self, Error> {
        let provider = boundary::provider(url, ethrpc_args);
        let chain =
            Chain::try_from(provider.get_chain_id().await?).map_err(|_| Error::UnsupportedChain)?;

        Ok(Self {
            provider,
            chain,
            url: url.clone(),
        })
    }

    /// Returns the chain for the RPC connection.
    pub fn chain(&self) -> Chain {
        self.chain
    }

    /// Returns a reference to the underlying provider.
    pub fn provider(&self) -> &AlloyProvider {
        &self.provider
    }

    /// Returns a reference to the underlying RPC URL.
    pub fn url(&self) -> &Url {
        &self.url
    }
}

/// The Ethereum blockchain.
#[derive(Clone)]
pub struct Ethereum {
    provider: AlloyProvider,
    unbuffered_provider: AlloyProvider,
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
        provider: AlloyProvider,
        unbuffered_provider: AlloyProvider,
        chain: &Chain,
        url: Url,
        addresses: contracts::Addresses,
        current_block_args: &shared::current_block::Arguments,
    ) -> Self {
        let contracts = Contracts::new(&provider, chain, addresses).await;

        Self {
            current_block: current_block_args
                .stream(url, unbuffered_provider.clone())
                .await
                .expect("couldn't initialize current block stream"),
            provider,
            unbuffered_provider,
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
        let (receipt, traces): (Option<TransactionReceipt>, GethTrace) = tokio::try_join!(
            self.provider
                .get_transaction_receipt(hash.0)
                .map_err(Error::Alloy),
            // Use unbuffered transport for the Debug API since not all providers support
            // batched debug calls.
            fetch_debug_trace(&self.unbuffered_provider, hash)
        )?;

        let receipt = receipt.ok_or(Error::TransactionNotFound)?;
        let block_hash =
            receipt
                .block_hash
                .ok_or(Error::IncompleteTransactionData(anyhow::anyhow!(
                    "missing block_hash"
                )))?;
        let block = self
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
async fn fetch_debug_trace(provider: &impl Provider, hash: eth::TxId) -> Result<GethTrace, Error> {
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
    GethTrace::deserialize(&mut de).map_err(Error::DeserializationFailed)
}

fn into_domain(
    receipt: TransactionReceipt,
    trace: GethTrace,
    timestamp: u64,
) -> anyhow::Result<domain::blockchain::Transaction> {
    let trace_calls = match trace {
        GethTrace::CallTracer(call_frame) => call_frame.into(),
        trace => bail!("unsupported trace call {trace:?}"),
    };

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
        trace_calls,
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
