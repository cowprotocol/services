use {
    self::solution::settlement,
    crate::{
        domain::{competition::solution::Settlement, liquidity},
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
    itertools::Itertools,
    rand::seq::SliceRandom,
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
    pub settlement: Mutex<Option<Settlement>>,
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

        // Encode the solutions into settlements.
        let settlements = join_all(solutions.map(|solution| async move {
            tracing::trace!(id = ?solution.id, "encoding settlement");
            (
                solution.id,
                solution.encode(auction, &self.eth, &self.simulator).await,
            )
        }))
        .await;

        // Filter out solutions that failed to encode.
        let mut settlements = settlements
            .into_iter()
            .filter_map(|(id, result)| {
                result
                    .tap_err(|err| {
                        tracing::info!(?err, ?id, "discarding solution: settlement encoding failed")
                    })
                    .ok()
            })
            .collect_vec();

        // TODO(#1483): parallelize this
        // TODO(#1480): more optimal approach for settlement merging

        // Merge the settlements in random order.
        settlements.shuffle(&mut rand::thread_rng());

        // The merging algorithm works as follows: the [`settlements`] vector keeps the
        // "most merged" settlements until they can't be merged anymore, at
        // which point they are pushed into the [`results`] vector.

        // The merged settlements in their final form.
        let mut results = Vec::new();
        while let Some(settlement) = settlements.pop() {
            // Has [`settlement`] been merged into another settlement?
            let mut merged = false;
            // Try to merge [`settlement`] into some other settlement.
            for other in settlements.iter_mut() {
                match other.merge(&settlement, &self.eth, &self.simulator).await {
                    Ok(m) => {
                        tracing::debug!(
                            settlement_1 = ?settlement.solutions(),
                            settlement_2 = ?other.solutions(),
                            "merged solutions"
                        );
                        *other = m;
                        merged = true;
                        break;
                    }
                    Err(err) => {
                        tracing::debug!(
                            ?err,
                            settlement_1 = ?settlement.solutions(),
                            settlement_2 = ?other.solutions(),
                            "solutions can't be merged"
                        );
                    }
                }
            }
            // If [`settlement`] can't be merged into any other settlement, this is its
            // final, most optimal form. Push it to the results.
            if !merged {
                results.push(settlement);
            }
        }

        let settlements = results;

        // Score the settlements.
        let scores = settlements
            .into_iter()
            .map(|settlement| {
                tracing::trace!(
                    solutions = ?settlement.solutions(),
                    settlement_id = ?settlement.id,
                    "scoring settlement"
                );
                (settlement.score(&self.eth, auction), settlement)
            })
            .collect_vec();

        // Filter out settlements which failed scoring.
        let scores = scores
            .into_iter()
            .filter_map(|(result, settlement)| {
                result
                    .tap_err(|err| {
                        tracing::info!(
                            ?err,
                            solutions = ?settlement.solutions(),
                            "discarding settlement: failed scoring"
                        );
                    })
                    .ok()
                    .map(|score| (score, settlement))
            })
            .collect_vec();

        // Trace the scores.
        for (score, settlement) in scores.iter() {
            tracing::info!(
                solutions = ?settlement.solutions(),
                settlement_id = ?settlement.id,
                score = score.0.to_f64_lossy(),
                "settlement scored"
            );
        }

        // Pick the best-scoring settlement.
        let (score, settlement) = scores
            .into_iter()
            .max_by_key(|(score, _)| score.to_owned())
            .ok_or(Error::SolutionNotFound)?;

        tracing::info!(?settlement, "winning settlement");

        let id = settlement.id;
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
        if id != settlement.id {
            return Err(Error::InvalidSolutionId);
        }
        tracing::trace!(?id, "settling");
        mempool::execute(&self.mempools, &self.solver, settlement)
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
