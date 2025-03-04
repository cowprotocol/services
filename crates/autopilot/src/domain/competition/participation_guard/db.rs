use {
    crate::{
        arguments::{
            DbBasedSolverParticipationGuardConfig,
            LowSettlingSolversFinderConfig,
            NonSettlingSolversFinderConfig,
        },
        domain::{Metrics, eth},
        infra::{self, solvers::dto},
    },
    chrono::{DateTime, Utc},
    ethrpc::block_stream::CurrentBlockWatcher,
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
                let banned_until = Utc::now() + self_.0.ttl;

                self_.post_process(
                    &non_settling_solvers,
                    dto::notify::BanReason::UnsettledConsecutiveAuctions,
                    found_at,
                    current_block,
                    banned_until,
                );
                self_.post_process(
                    &low_settling_solvers,
                    dto::notify::BanReason::HighSettleFailureRate,
                    found_at,
                    current_block,
                    banned_until,
                );
            }
            tracing::error!("stream of settlement updates terminated unexpectedly");
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
        ban_reason: dto::notify::BanReason,
        found_at_timestamp: Instant,
        found_at_block: u64,
        banned_until: DateTime<Utc>,
    ) {
        let non_settling_solver_names: Vec<&str> = solvers
            .iter()
            .filter_map(|solver| self.0.drivers_by_address.get(solver))
            .map(|driver| {
                Metrics::get()
                    .banned_solver
                    .with_label_values(&[driver.name.as_ref(), ban_reason.as_str()])
                    .inc();
                // Check if solver accepted this feature. This should be removed once the
                // CIP making this mandatory has been approved.
                if driver.requested_timeout_on_problems {
                    let is_absent_or_expired = self
                        .0
                        .banned_solvers
                        .get(&driver.submission_address)
                        .is_none_or(|entry| entry.elapsed() >= self.0.ttl);
                    // The solver should try again once the cache is expired.
                    if is_absent_or_expired {
                        tracing::debug!(solver = ?driver.name, "disabling solver temporarily");
                        infra::notify_banned_solver(driver.clone(), ban_reason, banned_until);
                        self.0
                            .banned_solvers
                            .insert(driver.submission_address, found_at_timestamp);
                    }
                }
                driver.name.as_ref()
            })
            .collect();

        if non_settling_solver_names.is_empty() {
            return;
        }

        let log_message = match ban_reason {
            dto::notify::BanReason::UnsettledConsecutiveAuctions => "found non-settling solvers",
            dto::notify::BanReason::HighSettleFailureRate => {
                "found high-failure-settlement solvers"
            }
        };
        tracing::debug!(solvers = ?non_settling_solver_names, ?found_at_block, log_message);
    }
}

#[async_trait::async_trait]
impl super::SolverValidator for SolverValidator {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        if let Some(entry) = self.0.banned_solvers.get(solver) {
            return Ok(entry.elapsed() >= self.0.ttl);
        }

        Ok(true)
    }
}
