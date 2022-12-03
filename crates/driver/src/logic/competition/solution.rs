use {
    super::auction::Auction,
    crate::{solver, solver::Solver},
    futures::future::try_join_all,
    nonempty::NonEmpty,
    num::BigRational,
};

/// A solution is a set of orders which the solver has found an optimal way to
/// settle. A [`Solution`] is generated in response to a
/// [`super::auction::Auction`].
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

/// Find the best-scored solution.
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
fn score(_solution: &Solution) -> Score {
    todo!()
}

/// A unique solution ID. TODO Once this is finally decided, document what this
/// ID is used for.
#[derive(Debug, Clone, Copy)]
pub struct Id(pub u64);
