//! Dispatches winning solutions back to their drivers for settlement.

use {
    crate::{
        domain::cycle::{Ranking, SolanaCycle},
        infra::{
            driver::{Driver, dto},
            inflight::InFlightOrders,
            observation::{Landed, SettlementWindows},
            sponsor::Sponsor,
        },
        run_loop::SettlementExecutor,
    },
    async_trait::async_trait,
    chain_types::solana::Pubkey,
    solana_sdk::clock::MAX_PROCESSING_AGE,
    std::{sync::Arc, time::Duration},
    tokio::sync::broadcast,
};

/// Mainnet's target slot duration, converting the landable slot window into
/// the wall-clock bound of a settle task.
const SLOT_DURATION: Duration = Duration::from_millis(400);

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
    /// Orders dispatched here are held out of auction cuts until their
    /// submission deadline passes.
    inflight: InFlightOrders,
}

impl DriverExecutor {
    pub fn new(
        drivers: Vec<Arc<Driver>>,
        windows: SettlementWindows,
        sponsor: Option<Sponsor>,
        inflight: InFlightOrders,
    ) -> Self {
        Self {
            drivers,
            windows,
            sponsor,
            inflight,
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
            // Held before the dispatch: the next cut must not re-auction
            // these orders while the settlement can still land.
            let uids: Vec<_> = winner.orders().iter().map(|order| order.uid).collect();
            self.inflight.hold(uids.iter().copied(), deadline);
            // A window that cannot be opened must not block the settlement,
            // the dispatch is the priority.
            if let Err(err) = self
                .windows
                .open_dispatched(auction_id, winner.solver(), winner.id(), *tip, deadline)
                .await
            {
                tracing::error!(auction_id, ?err, "failed to open the settlement window");
            }
            let landed_events = self.windows.landed_events();
            let inflight = self.inflight.clone();
            let solver = winner.solver();
            // The last instant the settlement transaction can land: the
            // deadline plus the blockhash lifetime, in wall-clock terms.
            let landable_until = tokio::time::Instant::now()
                + SLOT_DURATION
                    * u32::try_from(
                        deadline
                            .saturating_sub(*tip)
                            .saturating_add(MAX_PROCESSING_AGE as u64),
                    )
                    .unwrap_or(u32::MAX);
            tokio::spawn(async move {
                settle_task(
                    driver,
                    request,
                    landed_events,
                    inflight,
                    uids,
                    solver,
                    landable_until,
                )
                .await
            });
        }
    }
}

/// One dispatched settlement: race the driver response against the on-chain
/// observation and release the orders on the first evidence that no second
/// settlement can happen. A driver answer that proves nothing (the
/// transaction may be on the wire) keeps the orders held until the landing
/// is observed or nothing can land any more.
async fn settle_task(
    driver: Arc<Driver>,
    request: dto::SettleRequest,
    mut landed_events: broadcast::Receiver<Landed>,
    inflight: InFlightOrders,
    uids: Vec<chain_types::solana::IntentHash>,
    solver: Pubkey,
    landable_until: tokio::time::Instant,
) {
    let auction_id = request.auction_id;
    let deadline = request.submission_deadline_slot;
    let landed = wait_landed(&mut landed_events, auction_id, solver);
    tokio::pin!(landed);

    tokio::select! {
        () = &mut landed => {
            tracing::info!(
                driver = %driver.name,
                auction_id,
                "settlement observed on chain before the driver response"
            );
            inflight.release(uids.iter());
            return;
        }
        result = driver.settle(&request) => match result {
            Ok(response) => tracing::info!(
                driver = %driver.name,
                auction_id,
                deadline,
                tx_signature = %response.tx_signature,
                "settlement submitted"
            ),
            Err(err) if err.settlement_provably_unsent() => {
                tracing::error!(
                    driver = %driver.name,
                    auction_id,
                    ?err,
                    "settlement rejected before submission"
                );
                inflight.release(uids.iter());
                return;
            }
            Err(err) => tracing::error!(
                driver = %driver.name,
                auction_id,
                ?err,
                "settlement failed"
            ),
        },
    }

    // The driver's answer proves nothing about the transaction: release on
    // the observed landing, or let the hold expire once nothing can land.
    if tokio::time::timeout_at(landable_until, &mut landed)
        .await
        .is_ok()
    {
        inflight.release(uids.iter());
    }
}

/// Resolves when the auction's solver has a settlement observed on chain.
/// Pends forever when the channel closes, the caller's timeout bounds it.
async fn wait_landed(events: &mut broadcast::Receiver<Landed>, auction_id: i64, solver: Pubkey) {
    loop {
        match events.recv().await {
            Ok(landed) if landed.auction_id == auction_id && landed.solver == solver => return,
            Ok(_) => {}
            // Skipped messages cannot be recovered, later ones still arrive.
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => std::future::pending().await,
        }
    }
}
