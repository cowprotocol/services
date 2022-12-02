use {
    crate::{solver, solver::Solver},
    futures::future::try_join_all,
    nonempty::NonEmpty,
    num::BigRational,
};

/// An auction is a set of orders that can be solved. The solvers calculate
/// [`Solution`]s by picking subsets of these orders and solving them.
#[derive(Debug, Clone)]
pub struct Auction {
    pub id: AuctionId,
    // TODO This should contain the deadline as well
}

/// A solution is a set of orders which the solver has found an optimal to
/// settle.
#[derive(Debug)]
pub struct Solution {}

/// The solution score. This is often referred to as the "objective value".
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Score(BigRational);

impl From<Score> for BigRational {
    fn from(score: Score) -> Self {
        score.0
    }
}

/// Generate the best-scored solution.
pub async fn best(
    solvers: &NonEmpty<Solver>,
    auction: Auction,
) -> Result<(Solution, Score), solver::Error> {
    let solutions =
        try_join_all(solvers.iter().map(|solver| solver.solve(auction.clone()))).await?;
    Ok(solutions
        .into_iter()
        .map(|solution| {
            let score = score(&solution);
            (solution, score)
        })
        .max_by(|(_, lhs), (_, rhs)| lhs.cmp(rhs))
        .unwrap())
}

/// Calculate the score of a solution.
fn score(solution: &Solution) -> Score {
    todo!()
}

#[derive(Debug, Clone, Copy)]
pub struct AuctionId(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct SolverId(pub u64);

impl From<u64> for AuctionId {
    fn from(inner: u64) -> Self {
        Self(inner)
    }
}

impl From<u64> for SolverId {
    fn from(inner: u64) -> Self {
        Self(inner)
    }
}
