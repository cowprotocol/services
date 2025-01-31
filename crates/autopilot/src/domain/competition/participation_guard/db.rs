use {
    crate::{
        database::Postgres,
        domain::{eth, Metrics},
        infra,
    },
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::future::join_all,
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
        // Keep only drivers that accept unsettled blocking.
        let drivers_by_address = drivers_by_address
            .into_iter()
            .filter(|(_, driver)| driver.accepts_unsettled_blocking)
            .collect();
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
                match self_
                    .0
                    .db
                    .find_non_settling_solvers(self_.0.last_auctions_count, current_block)
                    .await
                {
                    Ok(non_settling_solvers) => {
                        let non_settling_solvers = non_settling_solvers
                            .into_iter()
                            .map(|solver| {
                                let address = eth::Address(solver.0.into());
                                if let Some(driver) = self_.0.drivers_by_address.get(&address) {
                                    Metrics::get()
                                        .non_settling_solver
                                        .with_label_values(&[&driver.name]);
                                }

                                address
                            })
                            .collect::<Vec<_>>();

                        tracing::debug!(?non_settling_solvers, "found non-settling solvers");
                        self_.notify_solvers(&non_settling_solvers);

                        let now = Instant::now();
                        non_settling_solvers
                            .into_iter()
                            // Check if solver accepted this feature. This should be removed once a CIP is
                            // approved.
                            .filter(|solver| self_.0.drivers_by_address.contains_key(solver))
                            .for_each(|solver| {
                                self_.0.banned_solvers.insert(solver, now);
                            });
                    }
                    Err(err) => {
                        tracing::warn!(?err, "error while searching for non-settling solvers")
                    }
                }
            }
        });
    }

    /// Try to notify all the non-settling solvers in a background task.
    fn notify_solvers(&self, non_settling_solvers: &[eth::Address]) {
        let futures = non_settling_solvers
            .iter()
            .cloned()
            .map(|solver| {
                let self_ = self.0.clone();
                async move {
                    if let Some(driver) = self_.drivers_by_address.get(&solver) {
                        if let Err(err) = driver
                            .notify(
                                &infra::solvers::dto::notify::Request::UnsettledConsecutiveAuctions,
                            )
                            .await
                        {
                            tracing::debug!(?solver, ?err, "unable to notify external solver");
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        tokio::spawn(async move {
            join_all(futures).await;
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
                self.0.banned_solvers.remove(&driver.submission_address);
            }
        }

        Ok(true)
    }
}
