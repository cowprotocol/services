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
        domain::{self, auction::order::Class},
        infra::{
            self,
            solvers::dto::{reveal, solve},
        },
        run::Liveness,
        run_loop::observe,
    },
    ::observe::metrics,
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{cmp, sync::Arc, time::Duration},
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
}

impl RunLoop {
    pub fn new(
        orderbook: infra::shadow::Orderbook,
        drivers: Vec<infra::Driver>,
        trusted_tokens: AutoUpdatingTokenList,
        solve_deadline: Duration,
        liveness: Arc<Liveness>,
    ) -> Self {
        Self {
            orderbook,
            drivers,
            trusted_tokens,
            auction: 0,
            block: 0,
            solve_deadline,
            liveness,
        }
    }

    pub async fn run_forever(mut self) -> ! {
        let mut previous = None;
        loop {
            let Some(domain::AuctionWithId { id, auction }) = self.next_auction().await else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };
            observe::log_auction_delta(id, &previous, &auction);
            self.liveness.auction();

            self.single_run(id, &auction)
                .instrument(tracing::info_span!("auction", id))
                .await;

            previous = Some(auction);
        }
    }

    async fn next_auction(&mut self) -> Option<domain::AuctionWithId> {
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
        if self.block == auction.auction.block {
            tracing::trace!("skipping already seen block");
            return None;
        }

        if auction
            .auction
            .orders
            .iter()
            .all(|order| match order.class {
                Class::Market => false,
                Class::Liquidity => true,
                Class::Limit => false,
            })
        {
            tracing::trace!("skipping empty auction");
            return None;
        }

        self.auction = auction.id;
        self.block = auction.auction.block;
        Some(auction)
    }

    async fn single_run(&self, id: domain::auction::Id, auction: &domain::Auction) {
        tracing::info!("solving");
        Metrics::get().auction.set(id);
        Metrics::get()
            .orders
            .set(i64::try_from(auction.orders.len()).unwrap_or(i64::MAX));

        let mut participants = self.competition(id, auction).await;

        // Shuffle so that sorting randomly splits ties.
        participants.shuffle(&mut rand::thread_rng());
        participants.sort_unstable_by_key(|participant| cmp::Reverse(participant.score()));

        if let Some(Participant {
            driver,
            solution: Ok(solution),
        }) = participants.first()
        {
            let reference_score = participants
                .get(1)
                .map(|participant| participant.score())
                .unwrap_or_default();
            let reward = solution
                .score
                .get()
                .checked_sub(reference_score)
                .expect("reference score unexpectedly larger than winner's score");

            tracing::info!(
                driver =% driver.name,
                score =% solution.score,
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
    async fn competition(
        &self,
        id: domain::auction::Id,
        auction: &domain::Auction,
    ) -> Vec<Participant<'_>> {
        let request =
            solve::Request::new(id, auction, &self.trusted_tokens.all(), self.solve_deadline);
        let request = &request;

        futures::future::join_all(self.drivers.iter().map(|driver| async move {
            let solution = self.participate(driver, request).await;
            Participant { driver, solution }
        }))
        .await
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
        let (score, solution_id, submission_address) = proposed
            .solutions
            .into_iter()
            .max_by_key(|solution| solution.score)
            .map(|solution| {
                (
                    solution.score,
                    solution.solution_id,
                    solution.submission_address,
                )
            })
            .ok_or(Error::NoSolutions)?;

        let score = NonZeroU256::new(score).ok_or(Error::ZeroScore)?;

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
        })
    }
}

struct Participant<'a> {
    driver: &'a infra::Driver,
    solution: Result<Solution, Error>,
}

struct Solution {
    score: NonZeroU256,
    account: H160,
    calldata: reveal::Calldata,
}

impl Participant<'_> {
    fn score(&self) -> U256 {
        self.solution
            .as_ref()
            .map(|solution| solution.score.get())
            .unwrap_or_default()
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
