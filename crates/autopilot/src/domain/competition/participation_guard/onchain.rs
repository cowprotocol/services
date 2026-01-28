use crate::{domain::eth, infra};

/// Calls Authenticator contract to check if a solver has a sufficient
/// permission.
pub(super) struct Validator {
    pub eth: infra::Ethereum,
}

impl Validator {
    pub async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        Ok(self
            .eth
            .contracts()
            .authenticator()
            .isSolver(*solver)
            .call()
            .await?)
    }
}
