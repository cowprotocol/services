mod liquorice;

use {
    crate::{
        boundary::notifier::liquorice::LiquoriceNotifier,
        domain::competition::solution::settlement::Settlement,
        infra,
    },
    anyhow::Result,
    futures::future::join_all,
};

/// Trait for notifying liquidity sources about auctions and settlements
#[async_trait::async_trait]
pub trait LiquiditySourcesNotifying: Send + Sync {
    async fn notify_before_settlement(&self, settlement: &Settlement) -> Result<()>;
}

pub struct Notifier {
    inner: Vec<Box<dyn LiquiditySourcesNotifying>>,
}

impl Notifier {
    pub fn try_new(
        config: &infra::notify::liquidity_sources::Config,
        chain: chain::Chain,
    ) -> Result<Self> {
        let mut inner: Vec<Box<dyn LiquiditySourcesNotifying>> = vec![];

        if let Some(liquorice) = &config.liquorice {
            inner.push(Box::new(LiquoriceNotifier::new(liquorice, chain)?));
        }

        Ok(Self { inner })
    }
}

#[async_trait::async_trait]
impl LiquiditySourcesNotifying for Notifier {
    async fn notify_before_settlement(&self, settlement: &Settlement) -> Result<()> {
        let futures = self
            .inner
            .iter()
            .map(|notifier| notifier.notify_before_settlement(settlement));

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
