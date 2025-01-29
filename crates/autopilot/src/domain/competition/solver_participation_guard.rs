use {
    crate::{database::Postgres, domain::eth, infra::Ethereum},
    ethrpc::block_stream::CurrentBlockWatcher,
    std::{
        sync::Arc,
        time::{Duration, Instant},
    },
};

#[derive(Clone)]
pub struct SolverParticipationGuard(Arc<Inner>);

struct Inner {
    onchain_solver_participation_validator: OnchainSolverParticipationValidator,
    database_solver_participation_validator: DatabaseSolverParticipationValidator,
}

impl SolverParticipationGuard {
    pub fn new(
        eth: Ethereum,
        db: Postgres,
        settlement_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        ttl: Duration,
        last_auctions_count: u32,
    ) -> Self {
        let current_block = eth.current_block().clone();
        let onchain_solver_participation_validator = OnchainSolverParticipationValidator { eth };
        let database_solver_participation_validator = DatabaseSolverParticipationValidator::new(
            db,
            current_block,
            settlement_updates_receiver,
            ttl,
            last_auctions_count,
        );

        Self(Arc::new(Inner {
            onchain_solver_participation_validator,
            database_solver_participation_validator,
        }))
    }

    pub async fn can_participate(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        if !self
            .0
            .database_solver_participation_validator
            .can_participate(solver)
            .await?
        {
            return Ok(false);
        }

        if !self
            .0
            .onchain_solver_participation_validator
            .can_participate(solver)
            .await?
        {
            return Ok(false);
        }

        Ok(true)
    }
}

#[async_trait::async_trait]
trait SolverParticipationValidator: Send + Sync {
    async fn can_participate(&self, solver: &eth::Address) -> anyhow::Result<bool>;
}

#[derive(Clone)]
pub struct DatabaseSolverParticipationValidator(Arc<DatabaseSolverParticipationValidatorInner>);

struct DatabaseSolverParticipationValidatorInner {
    db: Postgres,
    cache: dashmap::DashMap<eth::Address, Instant>,
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
            cache: Default::default(),
            ttl,
            last_auctions_count,
        }));

        self_.start_maintenance(settlement_updates_receiver, current_block);

        self_
    }

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
                            .map(|solver| eth::Address(solver.0.into()))
                            .collect::<Vec<_>>();

                        tracing::debug!(?non_settling_solvers, "found non-settling solvers");

                        let now = Instant::now();
                        for solver in non_settling_solvers {
                            self_.cache.insert(solver, now);
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
    async fn can_participate(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        if let Some(entry) = self.0.cache.get(solver) {
            if Instant::now().duration_since(*entry.value()) < self.0.ttl {
                return Ok(false);
            } else {
                self.0.cache.remove(solver);
            }
        }

        Ok(true)
    }
}

struct OnchainSolverParticipationValidator {
    eth: Ethereum,
}

#[async_trait::async_trait]
impl SolverParticipationValidator for OnchainSolverParticipationValidator {
    async fn can_participate(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        Ok(self
            .eth
            .contracts()
            .authenticator()
            .is_solver(solver.0)
            .call()
            .await?)
    }
}
