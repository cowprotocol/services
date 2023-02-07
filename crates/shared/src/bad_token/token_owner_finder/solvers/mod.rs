pub mod solver_api;
pub mod solver_finder;

use {anyhow::Result, ethcontract::H160, std::collections::HashMap};

type Token = H160;
type Owner = H160;

#[async_trait::async_trait]
pub trait TokenOwnerSolverApi: Send + Sync {
    /// Get token owner pairs from specific solver
    async fn get_token_owner_pairs(&self) -> Result<HashMap<Token, Vec<Owner>>>;
}
