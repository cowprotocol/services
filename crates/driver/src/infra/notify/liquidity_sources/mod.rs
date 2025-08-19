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
use futures::FutureExt;
pub mod config;
pub mod liquorice;

pub use config::Config;
use {
    crate::domain::competition::solution::settlement::Settlement,
    anyhow::bail,
    ethcontract::jsonrpc::futures_util::future::join_all,
    std::{collections::HashMap, sync::Arc},
};

type LiquiditySourcesNotifiers = HashMap<String, Box<dyn LiquiditySourcesNotifying>>;
type Inner = Arc<LiquiditySourcesNotifiers>;

const SOURCE_NAME_LIQUORICE: &str = "liquorice";

/// Trait for notifying liquidity sources about auctions and settlements
#[async_trait::async_trait]
pub trait LiquiditySourcesNotifying: Send + Sync {
    async fn settlement(&self, settlement: &Settlement) -> anyhow::Result<()>;
}

/// Auctions and settlement notifier for liquidity sources
#[derive(Clone)]
pub struct Notifier {
    inner: Inner,
}

impl Notifier {
    pub fn try_new(config: &Config, chain: chain::Chain) -> anyhow::Result<Self> {
        let mut notifiers = LiquiditySourcesNotifiers::default();

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
impl LiquiditySourcesNotifying for Notifier {
    /// Sends notification to liquidity sources before settlement
    async fn settlement(&self, settlement: &Settlement) -> anyhow::Result<()> {
        let futures = self.inner.iter().map(|(source_name, notifier)| {
            notifier
                .settlement(settlement)
                .map(|result| (source_name.to_string(), result))
        });

        let errors = join_all(futures)
            .await
            .into_iter()
            .filter_map(|(source_name, result)| match result {
                Ok(()) => None,
                Err(e) => Some((source_name, e)),
            })
            .collect::<HashMap<_, _>>();

        if !errors.is_empty() {
            bail!("{errors:?}")
        }

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
