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
    futures::{FutureExt, future::join_all},
    std::{collections::HashMap, sync::Arc},
};

type Notifiers = HashMap<String, Box<dyn LiquiditySourceNotifying>>;
type Inner = Arc<Notifiers>;

const SOURCE_NAME_LIQUORICE: &str = "liquorice";

/// Trait describing notifications send to liquidity source
#[async_trait::async_trait]
pub trait LiquiditySourceNotifying: Send + Sync {
    async fn settlement(&self, settlement: &Settlement) -> anyhow::Result<()>;
}

/// Aggregation of notifiers
#[derive(Clone)]
pub struct Notifier {
    inner: Inner,
}

impl Notifier {
    pub fn try_new(config: &Config, chain: chain::Chain) -> anyhow::Result<Self> {
        let mut notifiers = Notifiers::default();

        if let Some(liquorice) = &config.liquorice {
            notifiers.insert(
                SOURCE_NAME_LIQUORICE.to_string(),
                Box::new(liquorice::Notifier::new(liquorice, chain)?),
            );
        }

        Ok(Self {
            inner: Arc::new(notifiers),
        })
    }
}

#[async_trait::async_trait]
impl LiquiditySourceNotifying for Notifier {
    /// Sends notifications to liquidity sources on settlement
    async fn settlement(&self, settlement: &Settlement) -> anyhow::Result<()> {
        let futures = self.inner.iter().map(|(source_name, notifier)| {
            notifier
                .settlement(settlement)
                .inspect(|result| {
                    if let Err(error) = result {
                        tracing::warn!(
                            "Error notifying liquidity source '{}': {:?}",
                            source_name.clone(),
                            error
                        );
                    }
                })
                .map(|result| (source_name.to_string(), result))
        });

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
