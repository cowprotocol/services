use {
    crate::{domain::eth, infra},
    ethrpc::alloy::conversions::IntoAlloy,
};

/// Calls Authenticator contract to check if a solver has a sufficient
/// permission.
pub(super) struct Validator {
    pub eth: infra::Ethereum,
}

#[async_trait::async_trait]
impl super::SolverValidator for Validator {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        Ok(self
            .eth
            .contracts()
            .authenticator()
            .isSolver(solver.0.into_alloy())
            .call()
            .await?)
    }
}
