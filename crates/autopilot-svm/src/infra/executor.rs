//! Dispatches winning solutions back to their drivers for settlement.

use {
    crate::{
        domain::cycle::{Ranking, SolanaCycle},
        infra::{
            driver::{Driver, dto},
            observation::SettlementTracker,
        },
        run_loop::SettlementExecutor,
    },
    async_trait::async_trait,
    std::sync::Arc,
};

/// Slots a settlement may take after ranking before it counts as late.
/// TODO: make configurable.
const SUBMISSION_DEADLINE_SLOTS: u64 = 25;

/// Sends `/settle` to each winner's driver. Submission runs detached, the
/// loop starts the next cycle while settlements land.
pub struct DriverExecutor {
    drivers: Vec<Arc<Driver>>,
    /// Dispatched settlements the observation side resolves or times out.
    tracker: SettlementTracker,
}

impl DriverExecutor {
    pub fn new(drivers: Vec<Arc<Driver>>, tracker: SettlementTracker) -> Self {
        Self { drivers, tracker }
    }
}

#[async_trait]
impl SettlementExecutor<SolanaCycle> for DriverExecutor {
    fn submission_deadline(&self, tip: &u64) -> u64 {
        tip + SUBMISSION_DEADLINE_SLOTS
    }

    async fn execute(&self, auction_id: i64, ranking: &Ranking, deadline: u64) {
        for winner in ranking.inner.winners() {
            let key = (winner.solver(), winner.id());
            let Some(driver) = ranking
                .drivers
                .get(&key)
                .and_then(|&index| self.drivers.get(index))
            else {
                tracing::error!(solution_id = winner.id(), "winner without a driver");
                continue;
            };
            let driver = Arc::clone(driver);
            let request = dto::SettleRequest {
                auction_id,
                solution_id: winner.id(),
                submission_deadline_slot: deadline,
            };
            // A window that cannot be opened must not block the settlement,
            // the dispatch is the priority.
            if let Err(err) = self
                .tracker
                .dispatched(
                    auction_id,
                    winner.solver(),
                    winner.id(),
                    deadline.saturating_sub(SUBMISSION_DEADLINE_SLOTS),
                    deadline,
                )
                .await
            {
                tracing::error!(auction_id, ?err, "failed to open the settlement window");
            }
            tokio::spawn(async move {
                match driver.settle(&request).await {
                    Ok(response) => tracing::info!(
                        driver = %driver.name,
                        auction_id,
                        deadline,
                        tx_signature = %response.tx_signature,
                        "settlement submitted"
                    ),
                    Err(err) => tracing::error!(
                        driver = %driver.name,
                        auction_id,
                        ?err,
                        "settlement failed"
                    ),
                }
            });
        }
    }
}
