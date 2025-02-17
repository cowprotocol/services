use {
    crate::{
        domain::{eth, Metrics},
        infra,
    },
    chrono::Utc,
    ethrpc::block_stream::CurrentBlockWatcher,
    std::{
        collections::HashMap,
        sync::Arc,
        time::{Duration, Instant},
    },
};

/// Checks the DB by searching for solvers that won N last consecutive auctions
/// but never settled any of them.
#[derive(Clone)]
pub(super) struct Validator(Arc<Inner>);

struct Inner {
    persistence: infra::Persistence,
    banned_solvers: dashmap::DashMap<eth::Address, Instant>,
    ttl: Duration,
    last_auctions_count: u32,
    drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
}

impl Validator {
    pub fn new(
        persistence: infra::Persistence,
        current_block: CurrentBlockWatcher,
        competition_updates_receiver: tokio::sync::mpsc::UnboundedReceiver<()>,
        ttl: Duration,
        last_auctions_count: u32,
        drivers_by_address: HashMap<eth::Address, Arc<infra::Driver>>,
    ) -> Self {
        let self_ = Self(Arc::new(Inner {
            persistence,
            banned_solvers: Default::default(),
            ttl,
            last_auctions_count,
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
                match self_
                    .0
                    .persistence
                    .find_non_settling_solvers(self_.0.last_auctions_count, current_block)
                    .await
                {
                    Ok(non_settling_solvers) => {
                        let non_settling_drivers = non_settling_solvers
                            .into_iter()
                            .filter_map(|solver| {
                                self_.0.drivers_by_address.get(&solver).map(|driver| {
                                    Metrics::get()
                                        .non_settling_solver
                                        .with_label_values(&[&driver.name]);

                                    driver.clone()
                                })
                            })
                            .collect::<Vec<_>>();

                        let non_settling_solver_names = non_settling_drivers
                            .iter()
                            .map(|driver| driver.name.clone())
                            .collect::<Vec<_>>();

                        tracing::debug!(solvers = ?non_settling_solver_names, "found non-settling solvers");

                        let non_settling_drivers = non_settling_drivers
                            .into_iter()
                            // Check if solver accepted this feature. This should be removed once a CIP is
                            // approved.
                            .filter(|driver| driver.accepts_unsettled_blocking)
                            .collect::<Vec<_>>();

                        let now = Instant::now();
                        let banned_until = Utc::now() + self_.0.ttl;
                        infra::notify_non_settling_solvers(&non_settling_drivers, banned_until);

                        for driver in non_settling_drivers {
                            self_
                                .0
                                .banned_solvers
                                .insert(driver.submission_address, now);
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
