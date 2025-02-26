use {
    crate::{
        domain::{Metrics, eth},
        infra,
    },
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
                let non_settling_solvers = match self_
                    .0
                    .persistence
                    .find_non_settling_solvers(self_.0.last_auctions_count, current_block)
                    .await
                {
                    Ok(non_settling_solvers) => non_settling_solvers,
                    Err(err) => {
                        tracing::warn!(?err, "error while searching for non-settling solvers");
                        continue;
                    }
                };

                let now = Instant::now();
                let non_settling_solver_names: Vec<&str> = non_settling_solvers
                    .iter()
                    .filter_map(|solver| self_.0.drivers_by_address.get(solver))
                    .map(|driver| {
                        Metrics::get()
                            .non_settling_solver
                            .with_label_values(&[&driver.name]);
                        // Check if solver accepted this feature. This should be removed once the
                        // CIP making this mandatory has been approved.
                        if driver.requested_timeout_on_problems {
                            tracing::debug!(solver = ?driver.name, "disabling solver temporarily");
                            self_
                                .0
                                .banned_solvers
                                .insert(driver.submission_address, now);
                        }
                        driver.name.as_ref()
                    })
                    .collect();

                tracing::debug!(solvers = ?non_settling_solver_names, "found non-settling solvers");
            }
            tracing::error!("stream of settlement updates terminated unexpectedly");
        });
    }
}

#[async_trait::async_trait]
impl super::Validator for Validator {
    async fn is_allowed(&self, solver: &eth::Address) -> anyhow::Result<bool> {
        if let Some(entry) = self.0.banned_solvers.get(solver) {
            return Ok(entry.elapsed() >= self.0.ttl);
        }

        Ok(true)
    }
}
