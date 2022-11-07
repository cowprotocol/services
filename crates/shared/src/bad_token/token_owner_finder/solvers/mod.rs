pub mod solver_api;
pub mod solver_finder;

use std::collections::HashMap;

use anyhow::Result;
use ethcontract::H160;

type Token = H160;
type Owner = H160;

#[async_trait::async_trait]
pub trait TokenOwnerSolverApi: Send + Sync {
    /// Get token owner pairs from specific solver
    async fn get_token_owner_pairs(&self) -> Result<HashMap<Token, Vec<Owner>>>;
}
