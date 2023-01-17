use super::TokenOwnerSolverApi;
use anyhow::{Context, Result};
use ethcontract::H160;
use reqwest::{Client, Url};
use std::collections::HashMap;

type Token = H160;
type Owner = H160;

#[derive(Clone, Debug)]
pub struct SolverConfiguration {
    pub url: Url,
    pub client: Client,
}

impl SolverConfiguration {
    /// Return type is `Token, Option<Owner>` because there are
    /// entries containing `Null` instead of owner address.
    async fn query(&self) -> Result<HashMap<Token, Option<Owner>>> {
        let response = self
            .client
            .get(self.url.clone())
            .send()
            .await?
            .text()
            .await?;
        serde_json::from_str::<HashMap<Token, Option<Owner>>>(&response)
            .context(format!("bad query response: {}", response))
    }
}

#[async_trait::async_trait]
impl TokenOwnerSolverApi for SolverConfiguration {
    async fn get_token_owner_pairs(&self) -> Result<HashMap<Token, Vec<Owner>>> {
        self.query().await.map(|token_owner_pairs| {
            token_owner_pairs
                .into_iter()
                .filter_map(|(token, owner)| owner.map(|owner| (token, vec![owner])))
                .collect()
        })
    }
}
