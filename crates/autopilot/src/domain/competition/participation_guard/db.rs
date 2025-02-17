use {
    crate::{
        arguments::{
            DbBasedSolverParticipationGuardConfig,
            LowSettlingSolversFinderConfig,
            NonSettlingSolversFinderConfig,
        },
        domain::{eth, Metrics},
        infra::{self, solvers::dto},
    },
    ethrpc::block_stream::CurrentBlockWatcher,
    model::time::now_in_epoch_seconds,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::join,
};

/// Checks the DB by searching for solvers that won N last consecutive auctions
/// and either never settled any of them or their settlement success rate is
/// lower than `min_settlement_success_rate`.
#[derive(Clone)]
pub(super) struct SolverValidator(Arc<Inner>);

struct Inner {
    persistence: infra::Persistence,
    banned_solvers: dashmap::DashMap<eth::Address, Instant>,
    ttl: Duration,
    non_settling_config: NonSettlingSolversFinderConfig,
    low_settling_config: LowSettlingSolversFinderConfig,
    drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
}

impl SolverValidator {
    pub fn new(
        persistence: infra::Persistence,
        current_block: CurrentBlockWatcher,
        competition_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        db_based_validator_config: DbBasedSolverParticipationGuardConfig,
        drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
    ) -> Self {
        let self_ = Self(Arc::new(Inner {
            persistence,
            banned_solvers: Default::default(),
            ttl: db_based_validator_config.solver_blacklist_cache_ttl,
            non_settling_config: db_based_validator_config.non_settling_solvers_finder_config,
            low_settling_config: db_based_validator_config.low_settling_solvers_finder_config,
            drivers_by_address,
        }));

        self_.start_maintenance(competition_updates_receiver, current_block);

        self_
    }

    /// Update the internal cache only once the competition auctions table is
    /// updated to avoid redundant DB queries on each block or any other
    /// timeout.
    fn start_maintenance(
        &self,
        mut competition_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        current_block: CurrentBlockWatcher,
    ) {
        let self_ = self.clone();
        tokio::spawn(async move {
            while competition_updates_receiver.recv().await.is_some() {
                let current_block = current_block.borrow().number;

                let (non_settling_solvers, mut low_settling_solvers) = join!(
                    self_.find_non_settling_solvers(current_block),
                    self_.find_low_settling_solvers(current_block)
                );
                // Non-settling issue has a higher priority, remove duplicates from low-settling
                // solvers.
                low_settling_solvers.retain(|solver| !non_settling_solvers.contains(solver));

                let found_at = Instant::now();
                let banned_until_timestamp =
                    u64::from(now_in_epoch_seconds()) + self_.0.ttl.as_secs();

                self_.post_process(
                    &non_settling_solvers,
                    &dto::notify::BanReason::UnsettledConsecutiveAuctions,
                    found_at,
                    banned_until_timestamp,
                );
                self_.post_process(
                    &low_settling_solvers,
                    &dto::notify::BanReason::HighSettleFailureRate,
                    found_at,
                    banned_until_timestamp,
                );
            }
        });
    }

    async fn find_non_settling_solvers(&self, current_block: u64) -> HashSet<eth::Address> {
        if !self.0.non_settling_config.enabled {
            return Default::default();
        }

        match self
            .0
            .persistence
            .find_non_settling_solvers(
                self.0.non_settling_config.last_auctions_participation_count,
                current_block,
            )
            .await
        {
            Ok(solvers) => solvers.into_iter().collect(),
            Err(err) => {
                tracing::warn!(?err, "error while searching for non-settling solvers");
                Default::default()
            }
        }
    }

    async fn find_low_settling_solvers(&self, current_block: u64) -> HashSet<eth::Address> {
        if !self.0.low_settling_config.enabled {
            return Default::default();
        }

        match self
            .0
            .persistence
            .find_low_settling_solvers(
                self.0.low_settling_config.last_auctions_participation_count,
                current_block,
                self.0
                    .low_settling_config
                    .solver_max_settlement_failure_rate,
            )
            .await
        {
            Ok(solvers) => solvers.into_iter().collect(),
            Err(err) => {
                tracing::warn!(?err, "error while searching for low-settling solvers");
                Default::default()
            }
        }
    }

    /// Updates the cache and notifies the solvers.
    fn post_process(
        &self,
        solvers: &HashSet<eth::Address>,
        ban_reason: &dto::notify::BanReason,
        found_at: Instant,
        banned_until_timestamp: u64,
    ) {
        if solvers.is_empty() {
            return;
        }

        let drivers = solvers
            .iter()
            .filter_map(|solver| self.0.drivers_by_address.get(solver).cloned())
            .collect::<Vec<_>>();

        let log_message = match ban_reason {
            dto::notify::BanReason::UnsettledConsecutiveAuctions => "found non-settling solvers",
            dto::notify::BanReason::HighSettleFailureRate => {
                "found high-failure-settlement solvers"
            }
        };
        let solver_names = drivers
            .iter()
            .map(|driver| driver.name.as_ref())
            .collect::<Vec<_>>();
        tracing::debug!(solvers = ?solver_names, log_message);

        for solver in solver_names {
            Metrics::get()
                .banned_solver
                .with_label_values(&[solver, ban_reason.as_str()]);
        }

        let banned_drivers = drivers
            .into_iter()
            // Notify and block only solvers that accept unsettled blocking feature. This should be removed once a CIP is approved.
            .filter(|driver| driver.accepts_unsettled_blocking)
            .collect::<Vec<_>>();

        infra::notify_banned_solvers(&banned_drivers, ban_reason, banned_until_timestamp);

        for driver in banned_drivers {
            self.0
                .banned_solvers
                .insert(driver.submission_address, found_at);
        }
    }
}

#[async_trait::async_trait]
impl super::SolverValidator for SolverValidator {
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
