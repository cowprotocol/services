use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use anyhow::Result;
use ethcontract::H160;
use reqwest::{Client, Url};

use super::TokenOwnerProposing;

type Token = H160;
type Owner = H160;

#[derive(Clone, Debug)]
pub struct SeaSolverConfiguration {
    pub url: Url,
    pub client: Client,
    pub update_interval: Duration,
}

impl SeaSolverConfiguration {
    async fn query(&self) -> Result<HashMap<Token, Option<Owner>>> {
        Ok(self
            .client
            .get(self.url.clone())
            .send()
            .await?
            .json()
            .await?)
    }
}

#[derive(Debug, Default)]
pub struct AutoUpdatingSeaSolverTokenOwnerFinder {
    cache: Arc<RwLock<HashMap<Token, Vec<Owner>>>>,
}

impl AutoUpdatingSeaSolverTokenOwnerFinder {
    pub fn new(configuration: SeaSolverConfiguration) -> Self {
        let cache = Arc::new(RwLock::new(Default::default()));

        // spawn a background task to regularly update cache
        {
            let cache = cache.clone();
            let updater = async move {
                loop {
                    match configuration.query().await {
                        Ok(token_owner_pairs) => {
                            let mut w = cache.write().unwrap();
                            *w = token_owner_pairs
                                .into_iter()
                                .filter_map(|(token, owner)| {
                                    owner.map(|owner| (token, vec![owner]))
                                })
                                .collect();
                        }
                        Err(err) => tracing::error!(?err, "failed to update token list"),
                    }

                    tokio::time::sleep(configuration.update_interval).await;
                }
            };
            tokio::task::spawn(updater);
        }

        Self { cache }
    }
}

#[async_trait::async_trait]
impl TokenOwnerProposing for AutoUpdatingSeaSolverTokenOwnerFinder {
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
    use std::str::FromStr;

    use super::*;

    #[tokio::test]
    #[ignore]
    async fn seasolver_e2e_test() {
        let url = std::env::var("SEASOLVER_TOKEN_HOLDERS").unwrap();
        let configuration = SeaSolverConfiguration {
            url: Url::from_str(&url).unwrap(),
            client: Client::new(),
            update_interval: Duration::from_secs(1000),
        };
        let finder = AutoUpdatingSeaSolverTokenOwnerFinder::new(configuration);
        tokio::time::sleep(Duration::from_secs(10)).await;
        let owners = finder
            .find_candidate_owners(addr!("132d8D2C76Db3812403431fAcB00F3453Fc42125"))
            .await
            .unwrap();
        dbg!(owners);
    }
}
