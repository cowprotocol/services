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
        arguments::RunLoopMode,
        domain::{self, competition::TradedOrder},
        infra::{
            self,
            solvers::dto::{reveal, solve},
        },
        run::Liveness,
        run_loop::observe,
    },
    ::observe::metrics,
    ethrpc::block_stream::CurrentBlockWatcher,
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{
        cmp,
        collections::{HashMap, HashSet},
        sync::Arc,
        time::Duration,
    },
    tracing::Instrument,
};

pub struct RunLoop {
    orderbook: infra::shadow::Orderbook,
    drivers: Vec<infra::Driver>,
    trusted_tokens: AutoUpdatingTokenList,
    auction: domain::auction::Id,
    block: u64,
    solve_deadline: Duration,
    liveness: Arc<Liveness>,
    synchronization: RunLoopMode,
    current_block: CurrentBlockWatcher,
    max_winners_per_auction: usize,
}

impl RunLoop {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        orderbook: infra::shadow::Orderbook,
        drivers: Vec<infra::Driver>,
        trusted_tokens: AutoUpdatingTokenList,
        solve_deadline: Duration,
        liveness: Arc<Liveness>,
        synchronization: RunLoopMode,
        current_block: CurrentBlockWatcher,
        max_winners_per_auction: usize,
    ) -> Self {
        // Added to make sure no more than one winner is activated by accident
        // Supposed to be removed after the implementation of "multiple winners per
        // auction" is done
        assert_eq!(max_winners_per_auction, 1, "only one winner is supported");
        Self {
            orderbook,
            drivers,
            trusted_tokens,
            auction: 0,
            block: 0,
            solve_deadline,
            liveness,
            synchronization,
            current_block,
            max_winners_per_auction,
        }
    }

    pub async fn run_forever(mut self) -> ! {
        let mut previous = None;
        loop {
            if let RunLoopMode::SyncToBlockchain = self.synchronization {
                let _ = ethrpc::block_stream::next_block(&self.current_block).await;
            };
            let Some(auction) = self.next_auction().await else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            observe::log_auction_delta(&previous, &auction);
            self.liveness.auction();

            self.single_run(&auction)
                .instrument(tracing::info_span!("auction", auction.id))
                .await;

            previous = Some(auction);
        }
    }

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

    async fn single_run(&self, auction: &domain::Auction) {
        tracing::info!("solving");
        Metrics::get().auction.set(auction.id);
        Metrics::get()
            .orders
            .set(i64::try_from(auction.orders.len()).unwrap_or(i64::MAX));

        let participants = self.competition(auction).await;
        let winners = self.select_winners(&participants);

        for (i, Participant { driver, solution }) in winners.iter().enumerate() {
            let reference_score = winners
                .get(i + 1)
                .map(|winner| winner.score())
                .unwrap_or_default();
            let score = solution
                .as_ref()
                .map(|solution| solution.score.get())
                .unwrap_or_default();
            let reward = score
                .checked_sub(reference_score)
                .expect("reference score unexpectedly larger than winner's score");

            tracing::info!(
                driver =% driver.name,
                %score,
                %reward,
                "winner"
            );
            Metrics::get()
                .performance_rewards
                .with_label_values(&[&driver.name])
                .inc_by(reward.to_f64_lossy());
            Metrics::get().wins.with_label_values(&[&driver.name]).inc();
        }

        let hex = |bytes: &[u8]| format!("0x{}", hex::encode(bytes));
        for Participant { driver, solution } in participants {
            match solution {
                Ok(solution) => {
                    let uninternalized = (solution.calldata.internalized
                        != solution.calldata.uninternalized)
                        .then(|| hex(&solution.calldata.uninternalized));

                    tracing::debug!(
                        driver =% driver.name,
                        score =% solution.score,
                        account =? solution.account,
                        calldata =% hex(&solution.calldata.internalized),
                        ?uninternalized,
                        "participant"
                    );
                    Metrics::get()
                        .results
                        .with_label_values(&[&driver.name, "ok"])
                        .inc();
                }
                Err(err) => {
                    tracing::warn!(%err, driver =% driver.name, "driver error");
                    Metrics::get()
                        .results
                        .with_label_values(&[&driver.name, err.label()])
                        .inc();
                }
            };
        }
    }

    /// Runs the solver competition, making all configured drivers participate.
    async fn competition(&self, auction: &domain::Auction) -> Vec<Participant<'_>> {
        let request = solve::Request::new(auction, &self.trusted_tokens.all(), self.solve_deadline);
        let request = &request;

        let mut participants =
            futures::future::join_all(self.drivers.iter().map(|driver| async move {
                let solution = self.participate(driver, request).await;
                Participant { driver, solution }
            }))
            .await;

        // Shuffle so that sorting randomly splits ties.
        participants.shuffle(&mut rand::thread_rng());
        participants.sort_unstable_by_key(|participant| cmp::Reverse(participant.score()));

        participants
    }

    /// Chooses the winners from the given participants.
    ///
    /// Participants are already sorted by their score (best to worst).
    ///
    /// Winners are selected one by one, starting from the best solution,
    /// until `max_winners_per_auction` is hit. The solution can become winner
    /// if it swaps tokens that are not yet swapped by any other already
    /// selected winner.
    fn select_winners<'a>(&self, participants: &'a [Participant<'a>]) -> Vec<&'a Participant<'a>> {
        let mut winners = Vec::new();
        let mut already_swapped_tokens = HashSet::new();
        for participant in participants.iter() {
            if let Ok(solution) = &participant.solution {
                let swapped_tokens = solution
                    .orders()
                    .iter()
                    .map(|(_, order)| (order.sell.token, order.buy.token))
                    .collect::<HashSet<_>>();
                if swapped_tokens.is_disjoint(&already_swapped_tokens) {
                    winners.push(participant);
                    already_swapped_tokens.extend(swapped_tokens);
                    if winners.len() >= self.max_winners_per_auction {
                        break;
                    }
                }
            }
        }
        winners
    }

    /// Computes a driver's solutions in the shadow competition.
    async fn participate(
        &self,
        driver: &infra::Driver,
        request: &solve::Request,
    ) -> Result<Solution, Error> {
        let proposed = tokio::time::timeout(self.solve_deadline, driver.solve(request))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(Error::Solve)?;
        let (score, solution_id, submission_address, orders) = proposed
            .solutions
            .into_iter()
            .max_by_key(|solution| solution.score)
            .map(|solution| {
                (
                    solution.score,
                    solution.solution_id,
                    solution.submission_address,
                    solution.orders,
                )
            })
            .ok_or(Error::NoSolutions)?;

        let score = NonZeroU256::new(score).ok_or(Error::ZeroScore)?;
        let orders = orders
            .into_iter()
            .map(|(order_uid, amounts)| (order_uid.into(), amounts.into_domain()))
            .collect();

        let revealed = driver
            .reveal(&reveal::Request { solution_id })
            .await
            .map_err(Error::Reveal)?;
        if !revealed
            .calldata
            .internalized
            .ends_with(&request.id.to_be_bytes())
        {
            return Err(Error::Mismatch);
        }

        Ok(Solution {
            score,
            account: submission_address,
            calldata: revealed.calldata,
            orders,
        })
    }
}

struct Participant<'a> {
    driver: &'a infra::Driver,
    solution: Result<Solution, Error>,
}

impl Participant<'_> {
    fn score(&self) -> U256 {
        self.solution
            .as_ref()
            .map(|solution| solution.score.get())
            .unwrap_or_default()
    }
}

struct Solution {
    score: NonZeroU256,
    account: H160,
    calldata: reveal::Calldata,
    orders: HashMap<domain::OrderUid, TradedOrder>,
}

impl Solution {
    fn orders(&self) -> &HashMap<domain::OrderUid, TradedOrder> {
        &self.orders
    }
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("the solver timed out")]
    Timeout,
    #[error("driver did not propose any solutions")]
    NoSolutions,
    #[error("the proposed a 0-score solution")]
    ZeroScore,
    #[error("the solver's revealed solution does not match the auction")]
    Mismatch,
    #[error("solve error: {0}")]
    Solve(anyhow::Error),
    #[error("reveal error: {0}")]
    Reveal(anyhow::Error),
}

impl Error {
    fn label(&self) -> &str {
        match self {
            Error::Timeout => "timeout",
            Error::NoSolutions => "no_solutions",
            Error::ZeroScore => "zero_score",
            Error::Mismatch => "mismatch",
            Error::Solve(_) => "error",
            Error::Reveal(_) => "error",
        }
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
    wins: prometheus::CounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(metrics::get_storage_registry()).unwrap()
    }
}
