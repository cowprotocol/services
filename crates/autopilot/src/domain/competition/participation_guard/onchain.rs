use crate::infra::{Driver, Ethereum};

/// Calls Authenticator contract to check if a solver has a sufficient
/// permission.
pub(super) struct Validator {
    pub eth: Ethereum,
}

#[async_trait::async_trait]
impl super::Validator for Validator {
    async fn is_allowed(&self, solver: &Driver) -> anyhow::Result<bool> {
        Ok(self
            .eth
            .contracts()
            .authenticator()
            .is_solver(solver.submission_address.0)
            .call()
            .await?)
    }
}
