pub mod config;

use std::sync::Arc;
pub use config::Config;
use crate::{infra, boundary};
use crate::boundary::notifier::LiquiditySourcesNotifying;
use crate::domain::competition::solution::settlement::Settlement;

/// Auctions and settlement notifier for liquidity sources
#[derive(Debug, Clone)]
pub struct Notifier {
    inner: Arc<boundary::notifier::Notifier>,
}

impl Notifier {
    pub fn try_new(config: &infra::notify::liquidity_source::Config, chain_id: u64) -> Result<Self, Error> {
        Ok(Self {
            inner: Arc::new(boundary::notifier::Notifier::try_new(config, chain_id)?)
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
