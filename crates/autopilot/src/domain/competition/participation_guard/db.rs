use {
    crate::{
        database::Postgres,
        domain::{eth, Metrics},
        infra::{self, solvers::dto},
    },
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::future::join_all,
    std::{
        collections::{HashMap, HashSet},
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::join,
};

/// Checks the DB by searching for solvers that won N last consecutive auctions
/// but never settled any of them.
#[derive(Clone)]
pub(super) struct Validator(Arc<Inner>);

struct Inner {
    db: Postgres,
    banned_solvers: dashmap::DashMap<eth::Address, Instant>,
    ttl: Duration,
    last_auctions_count: u32,
    drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
}

impl Validator {
    pub fn new(
        db: Postgres,
        current_block: CurrentBlockWatcher,
        settlement_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        ttl: Duration,
        last_auctions_count: u32,
        drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
    ) -> Self {
        let self_ = Self(Arc::new(Inner {
            db,
            banned_solvers: Default::default(),
            ttl,
            last_auctions_count,
            drivers_by_address,
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
        let self_ = self.clone();
        tokio::spawn(async move {
            while settlement_updates_receiver.recv().await.is_some() {
                let current_block = current_block.borrow().number;

                let (non_settling_solvers, mut low_settling_solvers) = join!(
                    self_.find_non_settling_solvers(current_block),
                    self_.find_low_settling_solvers(current_block)
                );
                // Non-settling issue has a higher priority, remove duplicates from low-settling
                // solvers.
                low_settling_solvers.retain(|solver| !non_settling_solvers.contains(solver));

                self_.post_process(
                    &non_settling_solvers,
                    &dto::notify::Request::UnsettledConsecutiveAuctions,
                );
                self_.post_process(
                    &low_settling_solvers,
                    &dto::notify::Request::LowSettlingRate,
                );
            }
        });
    }

    async fn find_non_settling_solvers(&self, current_block: u64) -> HashSet<eth::Address> {
        let last_auctions_count = self.0.last_auctions_count;
        match self
            .0
            .db
            .find_non_settling_solvers(last_auctions_count, current_block)
            .await
        {
            Ok(solvers) => solvers
                .into_iter()
                .map(|solver| eth::Address(solver.0.into()))
                .collect(),
            Err(err) => {
                tracing::warn!(?err, "error while searching for non-settling solvers");
                Default::default()
            }
        }
    }

    async fn find_low_settling_solvers(&self, current_block: u64) -> HashSet<eth::Address> {
        let last_auctions_count = self.0.last_auctions_count;
        match self
            .0
            .db
            .find_low_settling_solvers(last_auctions_count, current_block, 1.0)
            .await
        {
            Ok(solvers) => solvers
                .into_iter()
                .map(|solver| eth::Address(solver.0.into()))
                .collect(),
            Err(err) => {
                tracing::warn!(?err, "error while searching for low-settling solvers");
                Default::default()
            }
        }
    }

    /// Try to notify all the non-settling solvers in a background task.
    fn notify_solvers(drivers: &[Arc<infra::Driver>], request: &dto::notify::Request) {
        let futures = drivers
            .iter()
            .cloned()
            .map(|driver| {
                let request = request.clone();
                async move {
                    if let Err(err) = driver.notify(&request).await {
                        tracing::debug!(solver = ?driver.name, ?err, "unable to notify external solver");
                    }
                }
            })
            .collect::<Vec<_>>();

        tokio::spawn(async move {
            join_all(futures).await;
        });
    }

    /// Updates the cache and notifies the solvers.
    fn post_process(&self, solvers: &HashSet<eth::Address>, request: &dto::notify::Request) {
        if solvers.is_empty() {
            return;
        }

        let drivers = solvers
            .iter()
            .filter_map(|solver| self.0.drivers_by_address.get(solver).cloned())
            .collect::<Vec<_>>();

        let log_message = match request {
            dto::notify::Request::UnsettledConsecutiveAuctions => "found non-settling solvers",
            dto::notify::Request::LowSettlingRate => "found low-settling solvers",
        };
        let solver_names = drivers
            .iter()
            .map(|driver| driver.name.clone())
            .collect::<Vec<_>>();
        tracing::debug!(solvers = ?solver_names, log_message);

        let reason = match request {
            dto::notify::Request::UnsettledConsecutiveAuctions => "non_settling",
            dto::notify::Request::LowSettlingRate => "low_settling",
        };

        for solver in solver_names {
            Metrics::get()
                .banned_solver
                .with_label_values(&[&solver, reason]);
        }

        let non_settling_drivers = drivers
            .into_iter()
            // Notify and block only solvers that accept unsettled blocking feature. This should be removed once a CIP is approved.
            .filter(|driver| driver.accepts_unsettled_blocking)
            .collect::<Vec<_>>();

        Self::notify_solvers(&non_settling_drivers, request);

        let now = Instant::now();
        for driver in non_settling_drivers {
            self.0.banned_solvers.insert(driver.submission_address, now);
        }
    }
}

#[async_trait::async_trait]
impl super::Validator for Validator {
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
