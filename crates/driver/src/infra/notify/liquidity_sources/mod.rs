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
pub mod liquorice;

pub use config::Config;
use {
    crate::domain::competition::solution::settlement::Settlement,
    ethcontract::jsonrpc::futures_util::future::join_all,
    std::sync::Arc,
};

type Inner = Arc<Vec<Box<dyn LiquiditySourcesNotifying>>>;

/// Trait for notifying liquidity sources about auctions and settlements
#[async_trait::async_trait]
pub trait LiquiditySourcesNotifying: Send + Sync {
    async fn notify_before_settlement(&self, settlement: &Settlement) -> anyhow::Result<()>;
}

/// Auctions and settlement notifier for liquidity sources
#[derive(Clone)]
pub struct Notifier {
    inner: Inner,
}

impl Notifier {
    pub fn try_new(config: &Config, chain: chain::Chain) -> anyhow::Result<Self> {
        let mut inner: Vec<Box<dyn LiquiditySourcesNotifying>> = vec![];

        if let Some(liquorice) = &config.liquorice {
            inner.push(Box::new(liquorice::Notifier::new(liquorice, chain)?));
        }

        Ok(Self {
            inner: Arc::new(inner),
        })
    }
}

#[async_trait::async_trait]
impl LiquiditySourcesNotifying for Notifier {
    /// Sends notification to liquidity sources before settlement
    async fn notify_before_settlement(&self, settlement: &Settlement) -> anyhow::Result<()> {
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
            .field("inner", &"LiquiditySources")
            .finish()
    }
}
