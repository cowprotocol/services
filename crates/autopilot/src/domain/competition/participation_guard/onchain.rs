use crate::{domain::eth, infra};

/// Calls Authenticator contract to check if a solver has a sufficient
/// permission.
pub(super) struct Validator {
    pub eth: infra::Ethereum,
}

#[async_trait::async_trait]
impl super::Validator for Validator {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        Ok(self
            .eth
            .contracts()
            .authenticator()
            .is_solver(solver.0)
            .call()
            .await?)
    }
}
