use {
    crate::{
        domain::eth,
        infra::blockchain::{self, Ethereum},
    },
    futures::future::join_all,
};

/// The mempool to use for publishing onchain transactions. The public mempool
/// of an [`Ethereum`] node can be used, or one of the private mempools offered
/// by various transaction relay services.
#[derive(Debug, Clone)]
pub struct Mempool(Inner);

impl Mempool {
    /// The [flashbots] private mempool.
    ///
    /// [flashbots]: https://docs.flashbots.net/flashbots-auction/overview
    pub fn flashbots() -> Self {
        Self(Inner::Flashbots)
    }

    /// The public mempool of an [`Ethereum`] node.
    pub fn public(eth: Ethereum) -> Self {
        Self(Inner::Public(eth))
    }

    /// Send a transaction using the mempool.
    pub async fn send(&self, tx: eth::Tx) -> Result<(), Error> {
        match &self.0 {
            Inner::Flashbots => todo!(),
            Inner::Public(eth) => eth.send_transaction(tx).await.map_err(Into::into),
        }
    }
}

pub async fn send(mempools: &[Mempool], tx: eth::Tx) -> Result<(), Error> {
    let results = join_all(mempools.iter().map(|mempool| {
        let tx = tx.clone();
        async move {
            let result = mempool.send(tx.clone()).await;
            if result.is_err() {
                tracing::warn!(?mempool, ?result, "sending transaction via mempool failed");
            }
            result
        }
    }))
    .await;
    if results.into_iter().all(|r| r.is_err()) {
        return Err(Error::AllMempoolsFailed);
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum Inner {
    Flashbots,
    Public(Ethereum),
}

#[derive(Debug, thiserror::Error)]
#[error("TODO")]
pub enum Error {
    #[error("blockchain error: {0:?}")]
    Blockchain(#[from] blockchain::Error),
    #[error("all mempools failed to send the transaction")]
    AllMempoolsFailed,
}
