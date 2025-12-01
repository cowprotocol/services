use {
    super::TokenOwnerSolverApi,
    alloy::primitives::Address,
    anyhow::{Context, Result},
    reqwest::{Client, Url},
    std::collections::HashMap,
};

type Token = Address;
type Owner = Address;

#[derive(Clone, Debug)]
pub struct SolverConfiguration {
    pub url: Url,
    pub client: Client,
}

#[async_trait::async_trait]
impl TokenOwnerSolverApi for SolverConfiguration {
    async fn get_token_owner_pairs(&self) -> Result<HashMap<Token, Vec<Owner>>> {
        let response = self
            .client
            .get(self.url.clone())
            .send()
            .await?
            .text()
            .await?;
        serde_json::from_str(&response).context(format!("bad query response: {response:?}"))
    }
}
