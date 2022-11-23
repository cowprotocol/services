use super::TokenOwnerSolverApi;
use crate::bad_token::token_owner_finder::TokenOwnerProposing;
use anyhow::Result;
use ethcontract::H160;
use prometheus::{
    core::{AtomicU64, GenericCounter},
    IntCounterVec,
};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    sync::{Arc, RwLock},
    time::Duration,
};

type Token = H160;
type Owner = H160;

#[derive(Debug)]
pub struct AutoUpdatingSolverTokenOwnerFinder {
    inner: Arc<Inner>,
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
struct Metrics {
    /// Tracks how often a token owner update succeeded or failed.
    #[metric(labels("identifier", "result"))]
    updates: IntCounterVec,
}

struct Inner {
    solver: Box<dyn TokenOwnerSolverApi>,
    cache: RwLock<HashMap<Token, Vec<Owner>>>,
    metrics: &'static Metrics,
    identifier: String,
}

impl Inner {
    pub fn get_update_counter(&self, success: bool) -> GenericCounter<AtomicU64> {
        let result = if success { "success" } else { "failure" };
        let labels = [&self.identifier, result];
        self.metrics.updates.with_label_values(&labels)
    }
}

impl AutoUpdatingSolverTokenOwnerFinder {
    pub fn new(
        solver: Box<dyn TokenOwnerSolverApi>,
        update_interval: Duration,
        identifier: String,
    ) -> Self {
        let inner = Arc::new(Inner {
            solver,
            cache: RwLock::new(Default::default()),
            metrics: Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
            identifier,
        });

        // reset metrics for consistent graphs in grafana
        inner.get_update_counter(true).reset();
        inner.get_update_counter(false).reset();

        // spawn a background task to regularly update cache
        {
            let inner = inner.clone();
            let updater = async move {
                loop {
                    let result = inner.update().await;
                    inner.get_update_counter(result.is_ok()).inc();
                    if let Err(err) = result {
                        tracing::warn!(?err, "failed to update token list");
                    }
                    tokio::time::sleep(update_interval).await;
                }
            };
            tokio::task::spawn(updater);
        }

        Self { inner }
    }

    pub async fn update(&self) -> Result<()> {
        self.inner.update().await
    }
}

impl Inner {
    async fn update(&self) -> Result<()> {
        let token_owner_pairs = self.solver.get_token_owner_pairs().await?;

        let mut cache = self.cache.write().unwrap();
        *cache = token_owner_pairs;

        Ok(())
    }
}

impl Debug for Inner {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("Inner").field("cache", &self.cache).finish()
    }
}

#[async_trait::async_trait]
impl TokenOwnerProposing for AutoUpdatingSolverTokenOwnerFinder {
    async fn find_candidate_owners(&self, token: Token) -> Result<Vec<Owner>> {
        Ok(self
            .inner
            .cache
            .read()
            .unwrap()
            .get(&token)
            .cloned()
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use reqwest::{Client, Url};
    use std::str::FromStr;

    use crate::bad_token::token_owner_finder::solvers::solver_api::SolverConfiguration;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn seasolver_e2e_test() {
        let url = std::env::var("SEASOLVER_TOKEN_HOLDERS").unwrap();
        let configuration = Box::new(SolverConfiguration {
            url: Url::from_str(&url).unwrap(),
            client: Client::new(),
        });
        let finder = AutoUpdatingSolverTokenOwnerFinder::new(
            configuration,
            Duration::from_secs(1000),
            "test".to_owned(),
        );
        tokio::time::sleep(Duration::from_secs(10)).await;
        let owners = finder
            .find_candidate_owners(addr!("132d8D2C76Db3812403431fAcB00F3453Fc42125"))
            .await
            .unwrap();
        dbg!(owners);
    }
}
