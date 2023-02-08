use {
    super::TokenOwnerSolverApi,
    anyhow::{Context, Result},
    ethcontract::H160,
    reqwest::{Client, Url},
    std::collections::HashMap,
};

type Token = H160;
type Owner = H160;

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
