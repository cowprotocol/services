//! This module implements the run-loop for the shadow autopilot.
//!
//! The shadow autopilot runs the solver competition using all configured
//! drivers using a configured up-stream deployment (in other words, it queries
//! the current `/api/v1/auction` from another CoW Protocol services deployment
//! and runs a solver competition with that auction, instead of building one).
//! The run-loop will report and log the winner **without** actually executing
//! any settlements on-chain.

use {
    crate::{
        domain::{
            self,
            competition::{Bid, Score, Unscored, winner_selection},
            eth::WrappedNativeToken,
        },
        infra::{
            self,
            solvers::dto::{reveal, solve},
        },
        run::Liveness,
        run_loop::observe,
    },
    ::observe::metrics,
    ::winner_selection::state::RankedItem,
    anyhow::Context,
    ethrpc::block_stream::CurrentBlockWatcher,
    itertools::Itertools,
    num::{CheckedSub, Saturating},
    shared::token_list::AutoUpdatingTokenList,
    std::{num::NonZeroUsize, sync::Arc, time::Duration},
    tracing::{Instrument, instrument},
};

pub struct RunLoop {
    orderbook: infra::shadow::Orderbook,
    drivers: Vec<Arc<infra::Driver>>,
    trusted_tokens: AutoUpdatingTokenList,
    auction: domain::auction::Id,
    block: u64,
    solve_deadline: Duration,
    liveness: Arc<Liveness>,
    current_block: CurrentBlockWatcher,
    winner_selection: winner_selection::Arbitrator,
}

impl RunLoop {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        orderbook: infra::shadow::Orderbook,
        drivers: Vec<Arc<infra::Driver>>,
        trusted_tokens: AutoUpdatingTokenList,
        solve_deadline: Duration,
        liveness: Arc<Liveness>,
        current_block: CurrentBlockWatcher,
        max_winners_per_auction: NonZeroUsize,
        weth: WrappedNativeToken,
    ) -> Self {
        Self {
            winner_selection: winner_selection::Arbitrator::new(
                max_winners_per_auction.get(),
                weth,
            ),
            orderbook,
            drivers,
            trusted_tokens,
            auction: 0,
            block: 0,
            solve_deadline,
            liveness,
            current_block,
        }
    }

    pub async fn run_forever(mut self) -> ! {
        let mut previous = None;
        loop {
            // We use this as a synchronization mechanism to sync the run loop starts with
            // the next mined block
            let start_block = ethrpc::block_stream::next_block(&self.current_block).await;
            let Some(auction) = self.next_auction().await else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            observe::log_auction_delta(&previous, &auction, &start_block);
            self.liveness.auction();

            self.single_run(&auction)
                .instrument(tracing::info_span!("auction", auction.id))
                .await;

            previous = Some(auction);
        }
    }

    #[instrument(skip_all)]
    async fn next_auction(&mut self) -> Option<domain::Auction> {
        let auction = match self.orderbook.auction().await {
            Ok(auction) => auction,
            Err(err) => {
                tracing::warn!(?err, "failed to retrieve auction");
                return None;
            }
        };

        if self.auction == auction.id {
            tracing::trace!("skipping already seen auction");
            return None;
        }
        if self.block == auction.block {
            tracing::trace!("skipping already seen block");
            return None;
        }

        if auction.orders.is_empty() {
            tracing::trace!("skipping empty auction");
            return None;
        }

        self.auction = auction.id;
        self.block = auction.block;
        Some(auction)
    }

    #[instrument(skip_all, fields(auction_id = auction.id))]
    async fn single_run(&self, auction: &domain::Auction) {
        tracing::info!("solving");
        Metrics::get().auction.set(auction.id);
        Metrics::get()
            .orders
            .set(i64::try_from(auction.orders.len()).unwrap_or(i64::MAX));

        let solutions = self.competition(auction).await;
        let ranking = self.winner_selection.arbitrate(solutions, auction);
        let scores = ranking.reference_scores();

        let total_score = ranking
            .winners()
            .map(|b| b.score())
            .reduce(Score::saturating_add)
            .unwrap_or_default();

        for bid in ranking.ranked() {
            let is_winner = bid.is_winner();
            let reference_score = scores.get(&bid.driver().submission_address);
            let driver = bid.driver();
            let reward = reference_score
                .map(|reference| {
                    total_score.checked_sub(reference).unwrap_or_else(|| {
                        tracing::trace!(
                            driver =% driver.name,
                            ?reference_score,
                            ?total_score,
                            "reference score exceeds total score, capping reward to 0"
                        );
                        Score::default()
                    })
                })
                .unwrap_or_default();

            tracing::info!(
                driver =% driver.name,
                ?reference_score,
                ?reward,
                %is_winner,
                "solution summary"
            );
            Metrics::get()
                .performance_rewards
                .with_label_values(&[&driver.name])
                .inc_by(f64::from(reward.get().0));
            Metrics::get()
                .wins
                .with_label_values(&[&driver.name])
                .inc_by(u64::from(is_winner))
        }
    }

    /// Runs the solver competition, making all configured drivers participate.
    #[instrument(skip_all)]
    async fn competition(&self, auction: &domain::Auction) -> Vec<Bid<Unscored>> {
        let request =
            solve::Request::new(auction, &self.trusted_tokens.all(), self.solve_deadline).await;

        futures::future::join_all(
            self.drivers
                .iter()
                .map(|driver| self.participate(Arc::clone(driver), request.clone(), auction.id)),
        )
        .await
        .into_iter()
        .flatten()
        .collect()
    }

    /// Computes a driver's solutions in the shadow competition.
    #[instrument(skip_all, fields(driver = driver.name))]
    async fn participate(
        &self,
        driver: Arc<infra::Driver>,
        request: solve::Request,
        auction_id: i64,
    ) -> Vec<Bid<Unscored>> {
        let solutions = match self.fetch_solutions(&driver, request).await {
            Ok(response) => {
                Metrics::get()
                    .results
                    .with_label_values(&[&driver.name, "ok"])
                    .inc();
                response.into_domain()
            }
            Err(err) => {
                Metrics::get()
                    .results
                    .with_label_values(&[&driver.name, "error"])
                    .inc();
                tracing::debug!(driver = driver.name, %err, "failed to fetch solutions");
                return vec![];
            }
        };

        let (solutions, errs): (Vec<_>, Vec<_>) = solutions.into_iter().partition_result();
        if !errs.is_empty() {
            tracing::debug!(len = errs.len(), ?errs, "dropping solutions with errors");
        }

        futures::future::join_all(solutions.iter().map(|s| async {
            let response = driver.reveal(reveal::Request {
                solution_id: s.id(),
                auction_id,
            })
            .await;
            let calldata = match response {
                Ok(response) => response.calldata.uninternalized,
                Err(err) => {
                    tracing::debug!(?err, driver = %driver.name, "failed to reveal calldata");
                    return;
                }
            };

            if !calldata.ends_with(&auction_id.to_be_bytes()) {
                tracing::warn!(driver = %driver.name, "solver did append auction id to the calldata");
            }
            tracing::debug!(
                driver = %driver.name,
                calldata = const_hex::encode_prefixed(calldata),
                "revealed calldata"
            );
        }))
        .await;

        solutions
            .into_iter()
            .map(|s| Bid::new(s, Arc::clone(&driver)))
            .collect()
    }

    #[instrument(skip_all)]
    async fn fetch_solutions(
        &self,
        driver: &infra::Driver,
        request: solve::Request,
    ) -> Result<solve::Response, anyhow::Error> {
        tokio::time::timeout(self.solve_deadline, driver.solve(request))
            .await
            .context("timeout")?
            .context("solve_request_failed")
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "shadow")]
struct Metrics {
    /// Tracks the last seen auction.
    auction: prometheus::IntGauge,

    /// Tracks the number of orders in the auction.
    orders: prometheus::IntGauge,

    /// Tracks the result of every driver.
    #[metric(labels("driver", "result"))]
    results: prometheus::IntCounterVec,

    /// Tracks the approximate performance rewards per driver
    #[metric(labels("driver"))]
    performance_rewards: prometheus::CounterVec,

    /// Tracks the winner of every auction.
    #[metric(labels("driver"))]
    wins: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(metrics::get_storage_registry()).unwrap()
    }
}
