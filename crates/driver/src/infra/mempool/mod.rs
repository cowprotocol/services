use {
    crate::{
        boundary::{Web3, unbuffered_web3},
        domain::tx_receiver::{NonceSource, SignedTx, TxReceiver},
    },
    alloy::{
        consensus::Transaction,
        eips::{BlockNumberOrTag, eip1559::Eip1559Estimation},
        providers::{Provider, ext::TxPoolApi},
        transports::TransportError,
    },
    anyhow::Context,
    dashmap::DashMap,
    eth_domain_types as eth,
    std::sync::Arc,
    url::Url,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub target_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    /// The block to read nonces at. `None` leaves the choice to the node.
    pub nonce_block_number: Option<BlockNumberOrTag>,
    pub url: Url,
    pub name: String,
    pub revert_protection: RevertProtection,
}

/// Don't submit transactions with high revert risk (i.e. transactions
/// that interact with on-chain AMMs) to the public mempool.
/// This can be enabled to avoid MEV when private transaction
/// submission strategies are available. If private submission strategies
/// are not available, revert protection is always disabled.
#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct Mempool {
    transport: Web3,
    config: Config,
    name: String,
    last_submissions: Arc<DashMap<eth::Address, Submission>>,
}

/// The last tx a signer sent to the mempool.
#[derive(Debug, Clone, Copy)]
struct Submission {
    nonce: u64,
    gas_price: Eip1559Estimation,
}

impl Mempool {
    pub fn new(config: Config) -> Self {
        Self {
            transport: unbuffered_web3(&config.url),
            name: format!("Mempool({})", config.name),
            config,
            last_submissions: Default::default(),
        }
    }

    /// Queries the mempool for a pending transaction of the given solver and
    /// nonce.
    async fn find_pending_tx_in_mempool(
        &self,
        signer: eth::Address,
        nonce: u64,
    ) -> anyhow::Result<Option<alloy::rpc::types::Transaction>> {
        let tx_pool_content = self
            .transport
            .provider
            .txpool_content_from(signer)
            .await
            .context("failed to query pending transactions")?;

        // find the one with the specified nonce
        let pending_tx = tx_pool_content
            .pending
            .into_iter()
            .chain(tx_pool_content.queued)
            .find(|(_signer, tx)| tx.nonce() == nonce)
            .map(|(_, tx)| tx);
        Ok(pending_tx)
    }
}

/// Reads nonces through the mempool's own URL.
#[async_trait::async_trait]
impl NonceSource for Mempool {
    async fn nonce(&self, signer: eth::Address) -> anyhow::Result<u64> {
        let call = self.transport.provider.get_transaction_count(signer);
        let nonce = match self.config.nonce_block_number {
            Some(BlockNumberOrTag::Latest) => call.latest(),
            Some(BlockNumberOrTag::Earliest) => call.earliest(),
            Some(BlockNumberOrTag::Finalized) => call.finalized(),
            Some(BlockNumberOrTag::Number(number)) => call.number(number),
            Some(BlockNumberOrTag::Pending) => call.pending(),
            Some(BlockNumberOrTag::Safe) => call.safe(),
            None => call,
        }
        .await?;
        Ok(nonce)
    }
}

#[async_trait::async_trait]
impl TxReceiver for Mempool {
    fn name(&self) -> &str {
        &self.name
    }

    async fn send(&self, tx: &SignedTx) -> anyhow::Result<()> {
        // The returned handle only watches for the receipt, which the caller
        // does itself.
        let result = self
            .transport
            .provider
            .send_raw_transaction(&tx.raw)
            .await
            .inspect_err(log_rpc_error_code);

        if let Err(err) = result {
            // log pending tx in case we failed to replace a pending tx
            let last_submission = self.last_submissions.get(&tx.signer).map(|s| *s);
            tracing::debug!(
                ?err,
                new_gas_price = ?tx.gas_price,
                nonce = tx.nonce,
                ?last_submission,
                signer = ?tx.signer,
                "failed to submit tx to mempool"
            );
            return Err(err.into());
        }

        tracing::debug!(
            nonce = tx.nonce,
            gas_price = ?tx.gas_price,
            signer = ?tx.signer,
            "successfully submitted tx to mempool"
        );
        self.last_submissions.insert(
            tx.signer,
            Submission {
                nonce: tx.nonce,
                gas_price: tx.gas_price,
            },
        );
        Ok(())
    }

    fn mines_reverting_txs(&self) -> bool {
        matches!(self.config.revert_protection, RevertProtection::Disabled)
    }

    async fn pending_gas_price(
        &self,
        signer: eth::Address,
        nonce: u64,
    ) -> Option<Eip1559Estimation> {
        if let Some(last_submission) = self.last_submissions.get(&signer) {
            return (last_submission.nonce == nonce).then_some(last_submission.gas_price);
        }

        // If we don't have the last submission in-memory (i.e. first
        // submission attempt after a restart) we try to inspect
        // the nodes transaction mempool. This is only done as a
        // backup since it can incur significant latency and
        // is generally not very widely supported.
        let pending_tx = self
            .find_pending_tx_in_mempool(signer, nonce)
            .await
            .inspect_err(|err| tracing::debug!(?err, "could not inspect tx mempool"))
            .ok()??;
        Some(Eip1559Estimation {
            max_fee_per_gas: pending_tx.max_fee_per_gas(),
            max_priority_fee_per_gas: pending_tx.max_priority_fee_per_gas().or_else(|| {
                tracing::error!(tx = ?pending_tx.inner.tx_hash(), "pending tx is not EIP 1559");
                None
            })?,
        })
    }
}

/// Logs a hint for known JSON-RPC error codes. Codes in -32768..=-32000 mean
/// different things per chain, so only map new codes once they're understood.
fn log_rpc_error_code(err: &TransportError) {
    match err.as_error_resp().map(|resp| resp.code) {
        // Standard error, e.g. "replacement transaction underpriced".
        None | Some(-32000) => {}
        // A proxy in front of the sequencer rejected the tx (seen on Ink).
        // https://github.com/ethereum-optimism/infra/blob/a4d73e6c8afed6b2dcb007d1c6f26455df45599f/proxyd/backend.go#L140-L144
        Some(-32100) => tracing::warn!(
            "transaction rejected upstream (JSON-RPC -32100): the RPC's proxy refused it before \
             the sequencer"
        ),
        Some(code) => tracing::warn!("unexpected error code {code}"),
    }
}
