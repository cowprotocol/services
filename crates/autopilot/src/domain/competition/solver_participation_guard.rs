use {
    crate::{
        arguments::SolverParticipationGuardConfig,
        database::Postgres,
        domain::{eth, Metrics},
        infra::Ethereum,
    },
    ethrpc::block_stream::CurrentBlockWatcher,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// This struct checks whether a solver can participate in the competition by
/// using different validators.
#[derive(Clone)]
pub struct SolverParticipationGuard(Arc<Inner>);

struct Inner {
    /// Stores the validators in order they will be called.
    validators: Vec<Box<dyn SolverParticipationValidator + Send + Sync>>,
}

impl SolverParticipationGuard {
    pub fn new(
        eth: Ethereum,
        db: Postgres,
        settlement_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        config: SolverParticipationGuardConfig,
    ) -> Self {
        let mut validators: Vec<Box<dyn SolverParticipationValidator + Send + Sync>> = Vec::new();

        if config.db_based_validator.enabled {
            let current_block = eth.current_block().clone();
            let database_solver_participation_validator = DatabaseSolverParticipationValidator::new(
                db,
                current_block,
                settlement_updates_receiver,
                config.db_based_validator.solver_blacklist_cache_ttl,
                config
                    .db_based_validator
                    .solver_last_auctions_participation_count,
            );
            validators.push(Box::new(database_solver_participation_validator));
        }

        if config.onchain_based_validator.enabled {
            let onchain_solver_participation_validator =
                OnchainSolverParticipationValidator { eth };
            validators.push(Box::new(onchain_solver_participation_validator));
        }

        Self(Arc::new(Inner { validators }))
    }

    /// Checks if a solver can participate in the competition.
    /// Sequentially asks internal validators to avoid redundant RPC calls in
    /// the following order:
    /// 1. DatabaseSolverParticipationValidator - operates fast since it uses
    ///    in-memory cache.
    /// 2. OnchainSolverParticipationValidator - only then calls the
    ///    Authenticator contract.
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
trait SolverParticipationValidator: Send + Sync {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool>;
}

/// Checks the DB by searching for solvers that won N last consecutive auctions
/// but never settled any of them.
#[derive(Clone)]
pub struct DatabaseSolverParticipationValidator(Arc<DatabaseSolverParticipationValidatorInner>);

struct DatabaseSolverParticipationValidatorInner {
    db: Postgres,
    banned_solvers: dashmap::DashMap<eth::Address, Instant>,
    ttl: Duration,
    last_auctions_count: u32,
}

impl DatabaseSolverParticipationValidator {
    pub fn new(
        db: Postgres,
        current_block: CurrentBlockWatcher,
        settlement_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        ttl: Duration,
        last_auctions_count: u32,
    ) -> Self {
        let self_ = Self(Arc::new(DatabaseSolverParticipationValidatorInner {
            db,
            banned_solvers: Default::default(),
            ttl,
            last_auctions_count,
        }));

        self_.start_maintenance(settlement_updates_receiver, current_block);

        self_
    }

    /// Update the internal cache only once the settlement table is updated to
    /// avoid redundant DB queries.
    fn start_maintenance(
        &self,
        mut settlement_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        current_block: CurrentBlockWatcher,
    ) {
        let self_ = self.0.clone();
        tokio::spawn(async move {
            while settlement_updates_receiver.recv().await.is_some() {
                let current_block = current_block.borrow().number;
                match self_
                    .db
                    .find_non_settling_solvers(self_.last_auctions_count, current_block)
                    .await
                {
                    Ok(non_settling_solvers) => {
                        let non_settling_solvers = non_settling_solvers
                            .into_iter()
                            .map(|solver| {
                                let address = eth::Address(solver.0.into());

                                Metrics::get()
                                    .non_settling_solver
                                    .with_label_values(&[&format!("{:#x}", address.0)]);

                                address
                            })
                            .collect::<Vec<_>>();

                        tracing::debug!(?non_settling_solvers, "found non-settling solvers");

                        let now = Instant::now();
                        for solver in non_settling_solvers {
                            self_.banned_solvers.insert(solver, now);
                        }
                    }
                    Err(err) => {
                        tracing::warn!(?err, "error while searching for non-settling solvers")
                    }
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl SolverParticipationValidator for DatabaseSolverParticipationValidator {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        if let Some(entry) = self.0.banned_solvers.get(solver) {
            if Instant::now().duration_since(*entry.value()) < self.0.ttl {
                return Ok(false);
            } else {
                self.0.banned_solvers.remove(solver);
            }
        }

        Ok(true)
    }
}

/// Calls Authenticator contract to check if a solver has a sufficient
/// permission.
struct OnchainSolverParticipationValidator {
    eth: Ethereum,
}

#[async_trait::async_trait]
impl SolverParticipationValidator for OnchainSolverParticipationValidator {
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
