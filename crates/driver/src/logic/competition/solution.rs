use {
    super::auction::Auction,
    crate::{solver, solver::Solver},
    futures::future::try_join_all,
    nonempty::NonEmpty,
    num::{BigRational, ToPrimitive},
};

/// A solution represents a set of orders which the solver has found an optimal
/// way to settle. A [`Solution`] is generated in response to a
/// [`super::auction::Auction`].
#[derive(Debug)]
pub struct Solution {}

/// The solution score. This is often referred to as the "objective value".
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Score(BigRational);

impl From<Score> for f64 {
    fn from(score: Score) -> Self {
        score.0.to_f64().expect("value can be represented as f64")
    }
}

/// Find the best-scored solution.
pub async fn solve(solvers: &NonEmpty<Solver>, auction: Auction) -> Result<Score, solver::Error> {
    let solutions =
        try_join_all(solvers.iter().map(|solver| solver.solve(auction.clone()))).await?;
    Ok(solutions.iter().map(score).max().expect("NonEmpty"))
}

/// Calculate the score of a solution.
fn score(_solution: &Solution) -> Score {
    todo!()
}

/// A unique solution ID. TODO Once this is finally decided, document what this
/// ID is used for.
#[derive(Debug, Clone, Copy)]
pub struct Id(pub u64);
