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
        driver_api::Driver,
        driver_model::{reveal, solve},
        protocol,
        run_loop,
    },
    model::{
        auction::{Auction, AuctionId, AuctionWithId},
        order::OrderClass,
    },
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, U256},
    rand::seq::SliceRandom,
    shared::token_list::AutoUpdatingTokenList,
    std::{cmp, time::Duration},
    tracing::Instrument,
};

pub struct RunLoop {
    orderbook: protocol::Orderbook,
    drivers: Vec<Driver>,
    trusted_tokens: AutoUpdatingTokenList,
    auction: AuctionId,
    block: u64,
    score_cap: U256,
}

impl RunLoop {
    pub fn new(
        orderbook: protocol::Orderbook,
        drivers: Vec<Driver>,
        trusted_tokens: AutoUpdatingTokenList,
        score_cap: U256,
    ) -> Self {
        Self {
            orderbook,
            drivers,
            trusted_tokens,
            auction: 0,
            block: 0,
            score_cap,
        }
    }

    pub async fn run_forever(mut self) -> ! {
        loop {
            let Some(AuctionWithId { id, auction }) = self.next_auction().await else {
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            };

            self.single_run(id, auction)
                .instrument(tracing::info_span!("auction", id))
                .await;
        }
    }

    async fn next_auction(&mut self) -> Option<AuctionWithId> {
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
            .all(|order| match order.metadata.class {
                OrderClass::Market => false,
                OrderClass::Liquidity => true,
                OrderClass::Limit(_) => false,
            })
        {
            tracing::trace!("skipping empty auction");
            return None;
        }

        self.auction = auction.id;
        self.block = auction.auction.block;
        Some(auction)
    }

    async fn single_run(&self, id: AuctionId, auction: Auction) {
        tracing::info!("solving");
        Metrics::get().auction.set(id);

        let mut participants = self.competition(id, &auction).await;

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
                driver =% driver.url,
                score =% solution.score,
                %reward,
                "winner"
            );
            Metrics::get()
                .performance_rewards
                .with_label_values(&[driver.url.as_str()])
                .inc_by(reward.to_f64_lossy());
        }

        let hex = |bytes: &[u8]| format!("0x{}", hex::encode(bytes));
        for Participant { driver, solution } in participants {
            match solution {
                Ok(solution) => {
                    let uninternalized = (solution.calldata.internalized
                        != solution.calldata.uninternalized)
                        .then(|| hex(&solution.calldata.uninternalized));

                    tracing::debug!(
                        driver =% driver.url,
                        score =% solution.score,
                        account =? solution.account,
                        calldata =% hex(&solution.calldata.internalized),
                        ?uninternalized,
                        "participant"
                    );
                    Metrics::get()
                        .results
                        .with_label_values(&[driver.url.as_str(), "ok"])
                        .inc();
                }
                Err(err) => {
                    tracing::warn!(%err, driver =% driver.url, "driver error");
                    Metrics::get()
                        .results
                        .with_label_values(&[driver.url.as_str(), err.label()])
                        .inc();
                }
            };
        }
    }

    /// Runs the solver competition, making all configured drivers participate.
    async fn competition(&self, id: AuctionId, auction: &Auction) -> Vec<Participant<'_>> {
        let request =
            run_loop::solve_request(id, auction, &self.trusted_tokens.all(), self.score_cap);
        let request = &request;

        futures::future::join_all(self.drivers.iter().map(|driver| async move {
            let solution = self.participate(driver, request).await;
            Participant { driver, solution }
        }))
        .await
    }

    /// Computes a driver's solution in the shadow competition.
    async fn participate(
        &self,
        driver: &Driver,
        request: &solve::Request,
    ) -> Result<Solution, Error> {
        let proposed = tokio::time::timeout(run_loop::SOLVE_TIME_LIMIT, driver.solve(request))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(Error::Solve)?;
        let score = NonZeroU256::new(proposed.score).ok_or(Error::ZeroScore)?;

        let revealed = driver.reveal().await.map_err(Error::Reveal)?;
        if !revealed
            .calldata
            .internalized
            .ends_with(&request.id.to_be_bytes())
        {
            return Err(Error::Mismatch);
        }

        Ok(Solution {
            score,
            account: proposed.submission_address,
            calldata: revealed.calldata,
        })
    }
}

struct Participant<'a> {
    driver: &'a Driver,
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
            Error::ZeroScore => "empty",
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

    /// Tracks the result of every refunding loops.
    #[metric(labels("driver", "result"))]
    results: prometheus::IntCounterVec,

    /// Tracks the approximate performance rewards per driver
    #[metric(labels("driver"))]
    performance_rewards: prometheus::CounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}
