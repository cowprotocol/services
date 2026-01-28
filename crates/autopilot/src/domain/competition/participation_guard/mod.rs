mod onchain;

use {crate::infra, std::sync::Arc};

/// This struct checks whether a solver can participate in the competition by
/// using the onchain validator.
#[derive(Clone)]
pub struct SolverParticipationGuard(Arc<onchain::Validator>);

impl SolverParticipationGuard {
    pub fn new(eth: infra::Ethereum) -> Self {
        Self(Arc::new(onchain::Validator { eth }))
    }

    /// Checks if a solver can participate in the competition by calling the
    /// Authenticator contract.
    pub async fn can_participate(
        &self,
        solver: &crate::domain::eth::Address,
    ) -> anyhow::Result<bool> {
        self.0.is_allowed(solver).await
    }
}
