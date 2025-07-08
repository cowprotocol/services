/// Module implements notifications for third party liquidity sources
/// used by solvers.
///
/// Such notifications are useful when a liquidity source needs to know
/// about the settlement before it gets submitted on-chain.
///
/// For example, when PMMs (Private Market Makers) provide firm quotes, they
/// need to know as early as possible that their quote will be used for the
/// settlement. It is crucial for risk management and leads to better
/// pricing.
pub mod config;

pub use config::Config;
use {
    crate::{
        boundary::{self, notifier::LiquiditySourcesNotifying},
        domain::competition::solution::settlement::Settlement,
        infra,
    },
    std::sync::Arc,
};

/// Auctions and settlement notifier for liquidity sources
#[derive(Debug, Clone)]
pub struct Notifier {
    inner: Arc<boundary::notifier::Notifier>,
}

impl Notifier {
    pub fn try_new(
        config: &infra::notify::liquidity_sources::Config,
        chain: chain::Chain,
    ) -> Result<Self, Error> {
        Ok(Self {
            inner: Arc::new(boundary::notifier::Notifier::try_new(config, chain)?),
        })
    }

    /// Sends notification to liquidity sources before settlement
    pub async fn notify_before_settlement(&self, settlement: &Settlement) -> Result<(), Error> {
        let _ = self.inner.notify_before_settlement(settlement).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
}
