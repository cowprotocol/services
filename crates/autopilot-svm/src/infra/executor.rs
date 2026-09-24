//! Dispatches winning solutions back to their drivers for settlement.

use {
    crate::{
        domain::cycle::{Ranking, SolanaCycle},
        infra::{
            driver::{Driver, dto},
            observation::SettlementWindows,
            sponsor::Sponsor,
        },
        run_loop::SettlementExecutor,
    },
    async_trait::async_trait,
    std::sync::Arc,
};

/// Sends `/settle` to each winner's driver. Submission runs detached, the
/// loop starts the next cycle while settlements land.
pub struct DriverExecutor {
    drivers: Vec<Arc<Driver>>,
    /// Opens a settlement-execution window per dispatched settlement, which
    /// the observation side later resolves or times out.
    windows: SettlementWindows,
    /// Countersigns pending sponsored creations. Absent, winners dispatch
    /// without creations, and one containing a pending sponsored order fails
    /// at the driver.
    sponsor: Option<Sponsor>,
}

impl DriverExecutor {
    pub fn new(
        drivers: Vec<Arc<Driver>>,
        windows: SettlementWindows,
        sponsor: Option<Sponsor>,
    ) -> Self {
        Self {
            drivers,
            windows,
            sponsor,
        }
    }
}

#[async_trait]
impl SettlementExecutor<SolanaCycle> for DriverExecutor {
    async fn execute(&self, auction_id: i64, ranking: &Ranking, tip: &u64, deadline: u64) {
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
            tracing::info!(driver = %driver.name, solution = winner.id(), "executing solution");
            // Countersign the winner's pending sponsored creations. A winner
            // whose creations cannot land any more cannot settle, so it is
            // skipped rather than dispatched to fail.
            let creations = match &self.sponsor {
                Some(sponsor) => {
                    match sponsor
                        .countersign_creations(winner.orders().iter().map(|order| order.uid))
                        .await
                    {
                        Ok(creations) => creations,
                        Err(err) => {
                            tracing::warn!(
                                solution = winner.id(),
                                ?err,
                                "skipping winner, sponsored creations unavailable"
                            );
                            continue;
                        }
                    }
                }
                None => vec![],
            };
            let request = dto::SettleRequest {
                auction_id,
                solution_id: winner.id(),
                submission_deadline_slot: deadline,
                creations,
            };
            // A window that cannot be opened must not block the settlement,
            // the dispatch is the priority. The window carries the generated
            // solution uid, the driver request keeps the driver-local id its
            // solution cache is keyed by.
            let uid = ranking.uids.get(&key).copied().unwrap_or_else(|| {
                tracing::error!(solution_id = winner.id(), "winner without a uid");
                i64::MAX
            });
            if let Err(err) = self
                .windows
                .open_dispatched(auction_id, winner.solver(), uid, *tip, deadline)
                .await
            {
                tracing::error!(auction_id, ?err, "failed to open the settlement window");
            }
            let windows = self.windows.clone();
            let solver = winner.solver();
            tokio::spawn(async move {
                match driver.settle(&request).await {
                    Ok(response) => tracing::info!(
                        driver = %driver.name,
                        auction_id,
                        deadline,
                        tx_signature = %response.tx_signature,
                        "settlement submitted"
                    ),
                    // No transaction went out, so the window closes as
                    // rejected: the orders re-enter the next cut and nothing
                    // is left to observe.
                    Err(err) if err.settlement_provably_unsent() => {
                        tracing::warn!(
                            driver = %driver.name,
                            auction_id,
                            ?err,
                            "settlement rejected before submission"
                        );
                        if let Err(err) = windows.close_rejected(auction_id, solver, uid).await {
                            tracing::error!(
                                auction_id,
                                ?err,
                                "failed to close the settlement window"
                            );
                        }
                    }
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
