pub mod solver_api;
pub mod solver_finder;

use {alloy::primitives::Address, anyhow::Result, std::collections::HashMap};

type Token = Address;
type Owner = Address;

#[async_trait::async_trait]
pub trait TokenOwnerSolverApi: Send + Sync {
    /// Get token owner pairs from specific solver
    async fn get_token_owner_pairs(&self) -> Result<HashMap<Token, Vec<Owner>>>;
}
