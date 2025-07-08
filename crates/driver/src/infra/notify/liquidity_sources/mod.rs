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
