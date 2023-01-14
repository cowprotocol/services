use {
    crate::{boundary, domain::competition::solution::settlement},
    futures::{future::select_ok, FutureExt},
};

pub use crate::boundary::mempool::Config;

/// The mempool to use for publishing settlements onchain. The public mempool
/// of an [`Ethereum`] node can be used, or one of the private mempools offered
/// by various transaction relay services.
#[derive(Debug, Clone)]
pub struct Mempool(boundary::Mempool);

impl Mempool {
    /// The [flashbots] private mempool.
    ///
    /// [flashbots]: https://docs.flashbots.net/flashbots-auction/overview
    pub fn flashbots(config: Config, url: reqwest::Url) -> Result<Self, Error> {
        boundary::Mempool::flashbots(config, url)
            .map(Self)
            .map_err(Into::into)
    }

    /// The public mempool of an [`Ethereum`] node.
    pub fn public(config: Config) -> Self {
        Self(boundary::Mempool::public(config))
    }

    /// Send the settlement using this mempool.
    pub async fn send(&self, settlement: settlement::Simulated) -> Result<(), Error> {
        self.0.send(settlement).await.map_err(Into::into)
    }
}

pub async fn send(mempools: &[Mempool], settlement: settlement::Simulated) -> Result<(), Error> {
    select_ok(mempools.iter().map(|mempool| {
        let settlement = settlement.clone();
        async move {
            let result = mempool.send(settlement).await;
            if result.is_err() {
                tracing::warn!(?result, "sending transaction via mempool failed");
            }
            result
        }
        .boxed()
    }))
    .await
    .map_err(|_| Error::AllMempoolsFailed)?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
    #[error("all mempools failed to send the transaction")]
    AllMempoolsFailed,
}
