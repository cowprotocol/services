mod liquorice;

use anyhow::Result;
use futures::future::join_all;
use crate::boundary::notifier::liquorice::LiquoriceNotifier;
use crate::infra;
use crate::domain::competition::solution::settlement::Settlement;

/// Trait for notifying liquidity sources about auctions and settlements
#[async_trait::async_trait]
pub trait LiquiditySourcesNotifying: Send + Sync {
    async fn notify_before_settlement(
        &self,
        settlement: &Settlement,
    ) -> Result<()>;
}

pub struct Notifier {
    inner: Vec<Box<dyn LiquiditySourcesNotifying>>,
}

impl Notifier {
    pub fn try_new(config: &infra::notify::liquidity_sources::Config, chain_id: u64) -> Result<Self> {
        let mut inner: Vec<Box<dyn LiquiditySourcesNotifying>> = vec![];

        if let Some(liquorice) = &config.liquorice {
            inner.push(Box::new(LiquoriceNotifier::new(liquorice, chain_id)?));
        }

        Ok(Self { inner })
    }
}

#[async_trait::async_trait]
impl LiquiditySourcesNotifying for Notifier {
    async fn notify_before_settlement(&self, settlement: &Settlement) -> Result<()> {
        let futures = self.inner.iter().map(|notifier| {
            notifier.notify_before_settlement(settlement)
        });

        let _ = join_all(futures).await;

        Ok(())
    }
}

impl std::fmt::Debug for Notifier {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Notifier")
            .field("inner", &"LiquiditySourcesNotifier")
            .finish()
    }
}

