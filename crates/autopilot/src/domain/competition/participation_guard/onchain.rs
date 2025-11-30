use {crate::infra, alloy::primitives::Address};

/// Calls Authenticator contract to check if a solver has a sufficient
/// permission.
pub(super) struct Validator {
    pub eth: infra::Ethereum,
}

#[async_trait::async_trait]
impl super::SolverValidator for Validator {
    async fn is_allowed(&self, solver: &Address) -> anyhow::Result<bool> {
        Ok(self
            .eth
            .contracts()
            .authenticator()
            .isSolver(*solver)
            .call()
            .await?)
    }
}
