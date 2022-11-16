use super::TokenOwnerSolverApi;
use crate::bad_token::token_owner_finder::TokenOwnerProposing;
use anyhow::Result;
use ethcontract::H160;
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

struct Inner {
    solver: Box<dyn TokenOwnerSolverApi>,
    cache: RwLock<HashMap<Token, Vec<Owner>>>,
}

impl AutoUpdatingSolverTokenOwnerFinder {
    pub fn new(solver: Box<dyn TokenOwnerSolverApi>, update_interval: Duration) -> Self {
        let inner = Arc::new(Inner {
            solver,
            cache: RwLock::new(Default::default()),
        });

        // spawn a background task to regularly update cache
        {
            let inner = inner.clone();
            let updater = async move {
                loop {
                    if let Err(err) = inner.update().await {
                        tracing::error!(?err, "failed to update token list");
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
        let finder =
            AutoUpdatingSolverTokenOwnerFinder::new(configuration, Duration::from_secs(1000));
        tokio::time::sleep(Duration::from_secs(10)).await;
        let owners = finder
            .find_candidate_owners(addr!("132d8D2C76Db3812403431fAcB00F3453Fc42125"))
            .await
            .unwrap();
        dbg!(owners);
    }
}
