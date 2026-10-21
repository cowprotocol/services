use {
    alloy::{eips::eip1559::Eip1559Estimation, primitives::Bytes},
    eth_domain_types::{self as eth, TxId},
};

/// An endpoint that takes signed transactions, e.g. a node in front of the
/// public mempool or a private relay.
#[async_trait::async_trait]
pub trait TxReceiver: Send + Sync + std::fmt::Debug {
    /// Identifies the receiver in logs and metrics.
    fn name(&self) -> &str;

    /// Returns once the receiver accepted the tx.
    async fn send(&self, tx: &SignedTx) -> anyhow::Result<()>;

    /// The gas price of the tx `signer` has pending at `nonce`, if the
    /// receiver can tell.
    async fn pending_gas_price(
        &self,
        signer: eth::Address,
        nonce: u64,
    ) -> Option<Eip1559Estimation>;

    /// Whether a tx that reverts gets mined rather than dropped.
    fn mines_reverting_txs(&self) -> bool;
}

/// Reads the next nonce of a submission account. Not part of [`TxReceiver`]
/// because not every receiver can tell, e.g. block builders.
#[async_trait::async_trait]
pub trait NonceSource: Send + Sync {
    async fn nonce(&self, signer: eth::Address) -> anyhow::Result<u64>;
}

/// A transaction signed by one of the submission accounts.
#[derive(Debug, Clone)]
pub struct SignedTx {
    /// EIP-2718 encoding, the input of `eth_sendRawTransaction`.
    pub raw: Bytes,
    pub hash: TxId,
    pub signer: eth::Address,
    pub nonce: u64,
    pub gas_price: Eip1559Estimation,
}
