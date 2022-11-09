use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use super::TokenOwnerSolverApi;
use crate::bad_token::token_owner_finder::TokenOwnerProposing;
use anyhow::Result;
use ethcontract::H160;

type Token = H160;
type Owner = H160;

#[derive(Debug, Default)]
pub struct AutoUpdatingSolverTokenOwnerFinder {
    cache: Arc<RwLock<HashMap<Token, Vec<Owner>>>>,
}

impl AutoUpdatingSolverTokenOwnerFinder {
    pub fn new(solver: Box<dyn TokenOwnerSolverApi>, update_interval: Duration) -> Self {
        let cache = Arc::new(RwLock::new(Default::default()));

        // spawn a background task to regularly update cache
        {
            let cache = cache.clone();
            let updater = async move {
                loop {
                    match solver.get_token_owner_pairs().await {
                        Ok(token_owner_pairs) => {
                            let mut w = cache.write().unwrap();
                            *w = token_owner_pairs;
                        }
                        Err(err) => tracing::error!(?err, "failed to update token list"),
                    }
                    tokio::time::sleep(update_interval).await;
                }
            };
            tokio::task::spawn(updater);
        }

        Self { cache }
    }
}

#[async_trait::async_trait]
impl TokenOwnerProposing for AutoUpdatingSolverTokenOwnerFinder {
    async fn find_candidate_owners(&self, token: Token) -> Result<Vec<Owner>> {
        Ok(self
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
