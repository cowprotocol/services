use {
    self::solution::settlement,
    crate::{
        boundary,
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
    std::{collections::HashSet, sync::Mutex},
};

pub mod auction;
pub mod order;
pub mod solution;

pub use {
    auction::Auction,
    order::Order,
    solution::{Score, Solution, SolverTimeout},
};

/// An ongoing competition. There is one competition going on per solver at any
/// time. The competition stores settlements to solutions generated by the
/// driver, and allows them to be executed onchain when requested later. The
/// solutions expire after a certain amount of time, at which point trying to
/// use them will return an `[Error::SolutionNotFound]`.
#[derive(Debug)]
pub struct Competition {
    pub solver: Solver,
    pub eth: Ethereum,
    pub liquidity: infra::liquidity::Fetcher,
    pub simulator: Simulator,
    pub now: time::Now,
    pub mempools: Vec<Mempool>,
    pub settlement: Mutex<Option<(solution::Id, settlement::Simulated)>>,
}

impl Competition {
    /// Solve an auction as part of this competition.
    pub async fn solve(&self, auction: &Auction) -> Result<(solution::Id, solution::Score), Error> {
        tracing::trace!("fetching liquidity");
        let liquidity = self.liquidity.fetch(&Self::liquidity_pairs(auction)).await;
        tracing::trace!("solving");
        let solution = self
            .solver
            .solve(auction, &liquidity, auction.deadline.timeout(self.now)?)
            .await?;
        // TODO(#1009) Keep in mind that the driver needs to make sure that the solution
        // doesn't fail simulation. Currently this is the case, but this needs to stay
        // the same as this code changes.
        tracing::trace!("simulating");
        let settlement = solution
            .simulate(&self.eth, &self.simulator, auction)
            .await?;
        tracing::trace!("scoring");
        let score = settlement.score(&self.eth, auction).await?;
        let id = settlement.id();
        *self.settlement.lock().unwrap() = Some((id, settlement));
        Ok((id, score))
    }

    // TODO Rename this to settle()?
    /// Execute (settle) a solution generated as part of this competition.
    pub async fn settle(&self, solution_id: solution::Id) -> Result<(), Error> {
        let settlement = match self.settlement.lock().unwrap().take() {
            Some((id, settlement)) if id == solution_id => settlement,
            Some((id, _)) => {
                tracing::warn!(?id, ?solution_id, "execute with wrong id");
                return Err(Error::SolutionNotFound);
            }
            None => {
                tracing::warn!(?solution_id, "execute without solve");
                return Err(Error::SolutionNotFound);
            }
        };
        tracing::trace!(?solution_id, "settling");
        mempool::send(&self.mempools, &self.solver, settlement)
            .await
            .map_err(Into::into)
    }

    /// Returns token pairs for liquidity relevant to a solver competition for
    /// the specified auction.
    fn liquidity_pairs(auction: &Auction) -> HashSet<liquidity::TokenPair> {
        auction
            .orders
            .iter()
            .filter_map(|order| match order.kind {
                order::Kind::Market | order::Kind::Limit { .. } => {
                    liquidity::TokenPair::new(order.sell.token, order.buy.token)
                }
                order::Kind::Liquidity => None,
            })
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no solution found for given id")]
    SolutionNotFound,
    #[error("solution error: {0:?}")]
    Solution(#[from] solution::Error),
    #[error("mempool error: {0:?}")]
    Mempool(#[from] mempool::Error),
    #[error("boundary error: {0:?}")]
    Boundary(#[from] boundary::Error),
    #[error("{0:?}")]
    DeadlineExceeded(#[from] auction::DeadlineExceeded),
    #[error("solver error: {0:?}")]
    Solver(#[from] solver::Error),
}
