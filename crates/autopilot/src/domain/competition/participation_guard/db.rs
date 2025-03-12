use {
    crate::{
        arguments::DbBasedSolverParticipationGuardConfig,
        domain::{Metrics, auction, competition, eth},
        infra::{self, solvers::dto},
    },
    ethrpc::block_stream::CurrentBlockWatcher,
    std::{
        collections::{HashMap, HashSet, VecDeque},
        sync::Arc,
    },
    tokio::sync::{Mutex, mpsc},
};

/// Checks the DB by searching for solvers that won N last consecutive auctions
/// and either never settled any of them or their settlement success rate is
/// lower than `min_settlement_success_rate`.
#[derive(Clone)]
pub(super) struct SolverValidator(Arc<Inner>);

struct Inner {
    persistence: infra::Persistence,
    /// A map of banned solver addresses and the corresponding number of
    /// settlements still need to skip before the solver can participate in
    /// the competition again.
    banned_solvers: dashmap::DashMap<eth::Address, u32>,
    config: DbBasedSolverParticipationGuardConfig,
    drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
    competitions_tracker: Mutex<CompetitionsTracker>,
}

#[derive(Clone, Debug, Default)]
struct SolverCompetitionStats {
    total: u32,
    failed: u32,
}

impl SolverCompetitionStats {
    fn increment(&mut self, success: bool, high_failure_threshold: f64) {
        if success {
            self.total += 1;
        } else if !success && self.failure_rate() <= high_failure_threshold {
            // Keep the failure rate at the threshold level, so the solver is
            // able to recover fast with a single successful settlement.
            self.failed += 1;
            self.total += 1;
        }
    }

    fn decrement(&mut self, success: bool) {
        self.total = self.total.saturating_sub(1);
        if !success {
            self.failed = self.failed.saturating_sub(1);
        }
    }

    fn failure_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.failed as f64 / self.total as f64
        }
    }
}

/// Keeps track of the last N competitions metadata to find solvers with high
/// failure rates and consecutive failed settlements.
/// Keeping only the last N competitions metadata is considered more efficient
/// for solvers to recover from a temporary failure state.
struct CompetitionsTracker {
    /// A FIFO queue to store the last N competitions metadata.
    queue: VecDeque<competition::Metadata>,
    /// The maximum size of the queue where the oldest metadata will be removed.
    max_cache_size: usize,
    /// The threshold to consider the solver as a low-settling solver. Once the
    /// threshold is reached, it stops growing on consecutive failed settlements
    /// to make is possible for solver to recover with a single successful
    /// settlement.
    high_failure_threshold: f64,
    /// The minimum number of competitions the solver should win to be
    /// considered as low-settling.
    min_won_competitions: u32,
    /// The number of consecutive failed settlements to consider the solver as
    /// non-settling.
    non_settling_threshold: u32,
    /// The statistics of the solver's competition participation.
    solver_stats: HashMap<eth::Address, SolverCompetitionStats>,
}

impl CompetitionsTracker {
    fn new(
        max_size: u32,
        high_failure_threshold: f64,
        min_winning_competitions: u32,
        non_settling_threshold: u32,
    ) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_size as usize),
            max_cache_size: max_size as usize,
            high_failure_threshold,
            min_won_competitions: min_winning_competitions,
            non_settling_threshold,
            solver_stats: Default::default(),
        }
    }

    fn update(&mut self, metadata: competition::Metadata) {
        if self.queue.len() == self.max_cache_size {
            if let Some(removed) = self.queue.pop_front() {
                self.update_solver_stats(&removed, false);
            }
        }

        self.update_solver_stats(&metadata, true);
        self.solver_stats.get(&metadata.solver);
        self.queue.push_back(metadata);
    }

    fn update_solver_stats(&mut self, metadata: &competition::Metadata, add: bool) {
        let stats = self.solver_stats.entry(metadata.solver).or_default();

        if !add {
            stats.decrement(metadata.settled);
        } else {
            stats.increment(metadata.settled, self.high_failure_threshold);
        }
    }

    fn find_high_failure_solvers(&self) -> HashSet<eth::Address> {
        self.solver_stats
            .iter()
            .filter_map(|(solver, stats)| {
                (stats.total >= self.min_won_competitions
                    && stats.failure_rate() > self.high_failure_threshold)
                    .then_some(*solver)
            })
            .collect()
    }

    fn reached_high_failure_rate(&self, solver: &eth::Address) -> bool {
        self.solver_stats.get(solver).is_some_and(|stats| {
            stats.total >= self.min_won_competitions
                && stats.failure_rate() > self.high_failure_threshold
        })
    }

    /// Find solvers that failed to settle the last N consecutive competitions,
    /// in other words, blocked the protocol.
    fn find_consecutive_failed_solvers(&self) -> HashSet<eth::Address> {
        let mut auction_ids: HashSet<auction::Id> = Default::default();
        let mut solver_failures: HashMap<eth::Address, u32> = Default::default();
        for metadata in self.queue.iter().rev() {
            auction_ids.insert(metadata.auction_id);
            if auction_ids.len() > self.non_settling_threshold as usize {
                break;
            }

            if !metadata.settled {
                *solver_failures.entry(metadata.solver).or_insert(0) += 1;
            }
        }

        solver_failures
            .into_iter()
            .filter_map(|(solver, failures)| {
                (failures >= self.non_settling_threshold).then_some(solver)
            })
            .collect()
    }
}

impl SolverValidator {
    pub fn new(
        persistence: infra::Persistence,
        current_block: CurrentBlockWatcher,
        competition_updates_receiver: mpsc::UnboundedReceiver<competition::Metadata>,
        db_based_validator_config: DbBasedSolverParticipationGuardConfig,
        drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
    ) -> Self {
        let settlements_tracker = CompetitionsTracker::new(
            db_based_validator_config
                .low_settling_solvers_finder
                .last_auctions_participation_count,
            db_based_validator_config
                .low_settling_solvers_finder
                .solver_max_settlement_failure_rate,
            db_based_validator_config
                .low_settling_solvers_finder
                .min_wins_threshold,
            db_based_validator_config
                .non_settling_solvers_finder
                .last_auctions_participation_count,
        );
        let self_ = Self(Arc::new(Inner {
            persistence,
            banned_solvers: Default::default(),
            config: db_based_validator_config,
            drivers_by_address,
            competitions_tracker: Mutex::new(settlements_tracker),
        }));

        self_.start_maintenance(competition_updates_receiver, current_block);

        self_
    }

    /// At the first run, fetches latest settlements data from the DB to
    /// populate the in-memory cache. Then, updates the stats only once new
    /// settlement data is indexed.
    fn start_maintenance(
        &self,
        mut competition_updates_receiver: mpsc::UnboundedReceiver<competition::Metadata>,
        current_block: CurrentBlockWatcher,
    ) {
        let self_ = self.clone();
        tokio::spawn(async move {
            let current_block = current_block.borrow().number;
            self_.init(current_block).await;

            while let Some(metadata) = competition_updates_receiver.recv().await {
                tracing::debug!(?metadata, "competition metadata received");
                // Decrease the pending settlements counter for all banned solvers
                for mut entry in self_.0.banned_solvers.iter_mut() {
                    *entry.value_mut() = entry.value().saturating_sub(1);
                }
                let solver = metadata.solver;
                let mut lock = self_.0.competitions_tracker.lock().await;
                lock.update(metadata);

                if lock.reached_high_failure_rate(&solver) {
                    self_.ban_solver(&solver, dto::notify::BanReason::HighSettleFailureRate);
                }

                let consecutive_failed_solvers = lock.find_consecutive_failed_solvers();
                for solver in consecutive_failed_solvers {
                    self_.ban_solver(
                        &solver,
                        dto::notify::BanReason::UnsettledConsecutiveAuctions,
                    );
                }
            }

            tracing::error!("competition metadata receiver closed");
        });
    }

    async fn init(&self, current_block: u64) {
        let metadata = match self.fetch_last_competitions_metadata(current_block).await {
            Ok(metadata) => metadata,
            Err(err) => {
                tracing::error!(?err, "error while fetching last competitions metadata");
                return;
            }
        };
        let mut competitions_tracker = self.0.competitions_tracker.lock().await;
        metadata.into_iter().for_each(|m| {
            competitions_tracker.update(m);
        });

        let low_settling_solvers = competitions_tracker.find_high_failure_solvers();
        let consecutive_failed_solvers = competitions_tracker.find_consecutive_failed_solvers();

        for solver in low_settling_solvers {
            self.ban_solver(&solver, dto::notify::BanReason::HighSettleFailureRate);
        }

        for solver in consecutive_failed_solvers {
            self.ban_solver(
                &solver,
                dto::notify::BanReason::UnsettledConsecutiveAuctions,
            );
        }
    }

    fn ban_solver(&self, solver: &eth::Address, ban_reason: dto::notify::BanReason) {
        let Some(driver) = self.0.drivers_by_address.get(solver) else {
            // This can happen only when the solver was disabled, and autopilot fetched the
            // previous data from the DB after the restart.
            return;
        };

        let pending = self
            .0
            .banned_solvers
            .get(solver)
            .map(|i| *i)
            .unwrap_or_default();
        let ban_mechanism_enabled = match ban_reason {
            dto::notify::BanReason::HighSettleFailureRate => {
                self.0.config.low_settling_solvers_finder.enabled
            }
            dto::notify::BanReason::UnsettledConsecutiveAuctions => {
                self.0.config.non_settling_solvers_finder.enabled
            }
        };
        let banning_allowed = ban_mechanism_enabled && driver.requested_timeout_on_problems;

        if pending == 0 {
            // The metric is updated regardless the config is enabled to track the
            // statistics.
            Metrics::get()
                .banned_solver
                .with_label_values(&[driver.name.as_ref(), ban_reason.as_str()])
                .inc();

            if banning_allowed {
                self.0
                    .banned_solvers
                    .insert(*solver, self.0.config.solver_ban_settlements_count);

                tracing::debug!(solver = ?driver.name, reason = ?ban_reason, "disabling solver temporarily");
                infra::notify_banned_solver(driver.clone(), ban_reason);
            }
        }
    }

    async fn fetch_last_competitions_metadata(
        &self,
        current_block: u64,
    ) -> anyhow::Result<Vec<competition::Metadata>> {
        let required_competitions_count = self
            .0
            .config
            .low_settling_solvers_finder
            .last_auctions_participation_count
            .max(
                self.0
                    .config
                    .non_settling_solvers_finder
                    .last_auctions_participation_count,
            );

        self.0
            .persistence
            .fetch_last_competitions_metadata(required_competitions_count, current_block)
            .await
    }
}

#[async_trait::async_trait]
impl super::SolverValidator for SolverValidator {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        if let Some(ban_expiration_counter) = self.0.banned_solvers.get(solver) {
            return Ok(*ban_expiration_counter == 0);
        }

        Ok(true)
    }
}
