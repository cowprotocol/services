use {
    crate::liquidity::Liquidity,
    anyhow::Result,
    model::TokenPair,
    shared::{baseline_solver::BaseTokens, recent_block_cache::Block},
    std::{
        collections::HashSet,
        future::Future,
        sync::{Arc, OnceLock},
        time::Duration,
    },
    tracing::Instrument,
};

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
    liquidity_source: Arc<OnceLock<L>>,
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
        Metrics::get()
            .liquidity_enabled
            .with_label_values(&[label])
            .set(0);
        let liquidity_source = Arc::new(OnceLock::new());
        let inner = liquidity_source.clone();
        let inner_label = label.to_owned();
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
                            if inner.set(source).is_err() {
                                // should never happen but if it does we want to know about it ASAP
                                tracing::error!(
                                    source = inner_label,
                                    "liquidity source already initialized"
                                );
                            } else {
                                tracing::debug!("successfully initialised liquidity source");
                                Metrics::get()
                                    .liquidity_enabled
                                    .with_label_values(&[&inner_label])
                                    .inc();
                            }

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
        match self.liquidity_source.get() {
            Some(source) => source.get_liquidity(pairs, at_block).await,
            None => Ok(vec![]),
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Tracks whether or not the graph based liquidity is currently enabled.
    #[metric(labels("source"))]
    liquidity_enabled: prometheus::IntGaugeVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod test {
    use {
        super::*,
        futures::FutureExt,
        shared::recent_block_cache::Block,
        std::sync::atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn delayed_init() {
        struct FakeSource;
        #[async_trait::async_trait]
        impl LiquidityCollecting for FakeSource {
            async fn get_liquidity(
                &self,
                _pairs: HashSet<TokenPair>,
                _at_block: Block,
            ) -> Result<Vec<Liquidity>> {
                // Yield here to verify that fetching liquidity in uninitialised state
                // will never yield.
                tokio::task::yield_now().await;
                // Use specific error message to verify initialisation
                Err(anyhow::anyhow!("I am initialised"))
            }
        }

        const ATTEMPTS: usize = 3;
        let counter = Arc::new(AtomicUsize::new(0));

        let closure_counter = counter.clone();
        let init = move || {
            let closure_counter = closure_counter.clone();
            async move {
                let attempt = closure_counter.fetch_add(1, Ordering::SeqCst);
                if attempt + 1 >= ATTEMPTS {
                    Ok(FakeSource)
                } else {
                    Err(anyhow::anyhow!("init failed"))
                }
            }
        };

        let source = BackgroundInitLiquiditySource::new("fake", init, Duration::from_millis(10));
        let gauge = Metrics::get()
            .liquidity_enabled
            .with_label_values(&["fake"]);
        assert_eq!(gauge.get(), 0);

        let liquidity = source
            .get_liquidity(Default::default(), Block::Recent)
            .now_or_never();
        // As long as the liquidity source is not initialised `get_liquidity` returns
        // immediately with 0 liquidity.
        assert!(liquidity.unwrap().unwrap().is_empty());

        // wait until initialisation is finished
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // init loop ran expected number of times
        assert_eq!(counter.load(Ordering::SeqCst), ATTEMPTS);
        let liquidity = source
            .get_liquidity(Default::default(), Block::Recent)
            .await;
        assert_eq!(liquidity.unwrap_err().to_string(), "I am initialised");
        assert_eq!(gauge.get(), 1);
    }
}
