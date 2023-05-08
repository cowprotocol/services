use {
    self::solution::settlement,
    crate::{
        domain::liquidity,
        infra::{
            self,
            blockchain::Ethereum,
            mempool,
            solver::{self, Solver},
            time,
            Mempool,
            Simulator,
        },
    },
    futures::future::join_all,
    std::sync::Mutex,
    tap::TapFallible,
};

pub mod auction;
pub mod order;
pub mod solution;

pub use {
    auction::Auction,
    order::Order,
    solution::{Reward, Score, Solution, SolverTimeout},
};

/// An ongoing competition. There is one competition going on per solver at any
/// time. The competition stores settlements to solutions generated by the
/// driver, and allows them to be executed onchain when requested later. The
/// solutions expire after a certain amount of time, at which point trying to
/// use them will return an `[Error::InvalidSolutionId]`.
#[derive(Debug)]
pub struct Competition {
    pub solver: Solver,
    pub eth: Ethereum,
    pub liquidity: infra::liquidity::Fetcher,
    pub simulator: Simulator,
    pub now: time::Now,
    pub mempools: Vec<Mempool>,
    pub settlement: Mutex<Option<settlement::Verified>>,
}

impl Competition {
    /// Solve an auction as part of this competition.
    pub async fn solve(
        &self,
        auction: &Auction,
    ) -> Result<(settlement::Id, solution::Score, solution::Reward), Error> {
        tracing::trace!("fetching liquidity");
        let liquidity = self
            .liquidity
            .fetch(
                &auction
                    .orders
                    .iter()
                    .filter_map(|order| match order.kind {
                        order::Kind::Market | order::Kind::Limit { .. } => {
                            liquidity::TokenPair::new(order.sell.token, order.buy.token)
                        }
                        order::Kind::Liquidity => None,
                    })
                    .collect(),
            )
            .await;

        // Fetch the solutions from the solver.
        tracing::trace!("solving");
        let solutions = self
            .solver
            .solve(auction, &liquidity, auction.deadline.timeout(self.now)?)
            .await?;

        // Empty solutions aren't useful, so discard them.
        let solutions = solutions.into_iter().filter(|solution| {
            if solution.is_empty() {
                tracing::info!(id = ?solution.id, "discarding solution: empty");
                false
            } else {
                true
            }
        });

        // Verify the solutions, encoding them into settlements.
        let settlements = join_all(solutions.map(|solution| async move {
            tracing::trace!(id = ?solution.id, "verifying");
            (
                solution.id,
                solution.verify(&self.eth, &self.simulator, auction).await,
            )
        }))
        .await;

        // Filter out solutions that failed verification.
        let settlements = settlements.into_iter().filter_map(|(id, result)| {
            result
                .tap_err(|err| {
                    tracing::info!(?err, ?id, "discarding solution: failed verification")
                })
                .ok()
        });

        // Score the solutions.
        let scores = join_all(settlements.map(|settlement| async move {
            tracing::trace!(id = ?settlement.solution.id, "scoring");
            (match settlement.solution.score {
                Some(score) => Ok(score),
                None => settlement.score(&self.eth, auction).await,
            }, settlement)
        }))
        .await;

        // Filter out solutions which failed scoring.
        let scores = scores
            .into_iter()
            .filter_map(|(result, settlement)| {
                result
                    .tap_err(|err| {
                        tracing::info!(?err, id = ?settlement.solution.id, "discarding solution: failed scoring")
                    })
                    .ok()
                    .map(|score| (score, settlement))
            });

        // Trace the scores.
        let scores = scores.map(|(score, settlement)| {
            tracing::info!(id = ?settlement.solution.id, score = f64::from(score.clone()), "solution scored");
            (score, settlement)
        });

        // Pick the best-scoring settlement.
        let (score, settlement) = scores
            .max_by_key(|(score, _)| score.to_owned())
            .ok_or(Error::SolutionNotFound)?;

        let id = settlement.id();
        *self.settlement.lock().unwrap() = Some(settlement);

        let reward = solution::Reward {
            performance_address: self.solver.address(),
            participation_address: self.solver.address(),
        };
        Ok((id, score, reward))
    }

    /// Execute a settlement generated as part of this competition.
    pub async fn settle(&self, id: settlement::Id) -> Result<(), Error> {
        let settlement = self
            .settlement
            .lock()
            .unwrap()
            .take()
            .ok_or(Error::InvalidSolutionId)?;
        if id != settlement.id() {
            return Err(Error::InvalidSolutionId);
        }
        tracing::trace!(?id, "settling");
        mempool::send(&self.mempools, &self.solver, settlement)
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no solution found for given id")]
    InvalidSolutionId,
    #[error("no solution found for the auction")]
    SolutionNotFound,
    #[error("mempool error: {0:?}")]
    Mempool(#[from] mempool::Error),
    #[error("{0:?}")]
    DeadlineExceeded(#[from] auction::DeadlineExceeded),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
}
