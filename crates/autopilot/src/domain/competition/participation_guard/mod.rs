mod db;
mod onchain;

use {
    crate::{
        arguments::DbBasedSolverParticipationGuardConfig,
        domain::eth,
        infra::{self, Ethereum},
    },
    std::sync::Arc,
};

/// This struct checks whether a solver can participate in the competition by
/// using different validators.
#[derive(Clone)]
pub struct SolverParticipationGuard(Arc<Inner>);

struct Inner {
    /// Stores the validators in order they will be called.
    validators: Vec<Box<dyn Validator + Send + Sync>>,
}

impl SolverParticipationGuard {
    pub fn new(
        eth: Ethereum,
        persistence: infra::Persistence,
        competition_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        db_based_validator_config: DbBasedSolverParticipationGuardConfig,
        drivers: impl IntoIterator<Item = Arc<infra::Driver>>,
    ) -> Self {
        let mut validators: Vec<Box<dyn Validator + Send + Sync>> = Vec::new();

        if db_based_validator_config.enabled {
            let current_block = eth.current_block().clone();
            let database_solver_participation_validator = db::Validator::new(
                persistence,
                current_block,
                competition_updates_receiver,
                db_based_validator_config.solver_blacklist_cache_ttl,
                db_based_validator_config.solver_last_auctions_participation_count,
                drivers
                    .into_iter()
                    .map(|driver| (driver.submission_address, driver.clone()))
                    .collect(),
            );
            validators.push(Box::new(database_solver_participation_validator));
        }

        let onchain_solver_participation_validator = onchain::Validator { eth };
        validators.push(Box::new(onchain_solver_participation_validator));

        Self(Arc::new(Inner { validators }))
    }

    /// Checks if a solver can participate in the competition.
    /// Sequentially asks internal validators to avoid redundant RPC calls in
    /// the following order:
    /// 1. DB-based validator: operates fast since it uses in-memory cache.
    /// 2. Onchain-based validator: only then calls the Authenticator contract.
    pub async fn can_participate(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        for validator in &self.0.validators {
            if !validator.is_allowed(solver).await? {
                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[async_trait::async_trait]
trait Validator: Send + Sync {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool>;
}
