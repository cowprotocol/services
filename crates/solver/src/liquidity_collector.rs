use {
    crate::liquidity::Liquidity,
    anyhow::Result,
    model::TokenPair,
    shared::{baseline_solver::BaseTokens, recent_block_cache::Block},
    std::{collections::HashSet, future::Future, sync::Arc, time::Duration},
    tokio::sync::Mutex,
    tracing::Instrument,
};

#[mockall::automock]
#[async_trait::async_trait]
pub trait LiquidityCollecting: Send + Sync {
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>>;
}

pub struct LiquidityCollector {
    pub liquidity_sources: Vec<Box<dyn LiquidityCollecting>>,
    pub base_tokens: Arc<BaseTokens>,
}

#[async_trait::async_trait]
impl LiquidityCollecting for LiquidityCollector {
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>> {
        let pairs = self.base_tokens.relevant_pairs(pairs.into_iter());
        let futures = self
            .liquidity_sources
            .iter()
            .map(|source| source.get_liquidity(pairs.clone(), at_block));
        let amms: Vec<_> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        tracing::debug!("got {} AMMs", amms.len());
        Ok(amms)
    }
}

/// A liquidity source which might not be initialised on creation. Instead
/// initialisation gets retried in a background task over and over until it
/// succeeds. Until the liquidity source has been initialised no liquidity will
/// be provided.
pub struct BackgroundInitLiquiditySource<L> {
    liquidity_source: Arc<Mutex<Option<L>>>,
}

impl<L> BackgroundInitLiquiditySource<L> {
    /// Creates a new liquidity source which might only be initialized at a
    /// later point in time.
    pub fn new<I, F>(label: &str, init: I, retry_init_timeout: Duration) -> Self
    where
        I: Fn() -> F + Send + Sync + 'static,
        F: Future<Output = Result<L>> + Send,
        L: LiquidityCollecting + 'static,
    {
        let liquidity_source: Arc<Mutex<Option<L>>> = Default::default();
        let inner = liquidity_source.clone();
        tokio::task::spawn(
            async move {
                loop {
                    match init().await {
                        Err(err) => {
                            tracing::warn!(
                                "failed to initialise liquidity source; next init attempt in \
                                 {retry_init_timeout:?}: {err:?}"
                            );
                            tokio::time::sleep(retry_init_timeout).await;
                        }
                        Ok(source) => {
                            let _ = inner.lock().await.insert(source);
                            tracing::debug!("successfully initialised liquidity source");
                            break;
                        }
                    }
                }
            }
            .instrument(tracing::info_span!("init", source = label)),
        );

        Self { liquidity_source }
    }
}

#[async_trait::async_trait]
impl<L> LiquidityCollecting for BackgroundInitLiquiditySource<L>
where
    L: LiquidityCollecting,
{
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>> {
        // Use `try_lock` to not block caller when the lock is currently being held for
        // a potentially very slow init logic.
        let liquidity_source = match self.liquidity_source.try_lock() {
            Ok(lock) => lock,
            Err(_) => return Ok(vec![]),
        };

        match &*liquidity_source {
            Some(initialised_source) => initialised_source.get_liquidity(pairs, at_block).await,
            None => Ok(vec![]),
        }
    }
}
