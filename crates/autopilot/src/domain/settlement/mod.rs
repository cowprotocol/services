//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use {
    super::competition,
    crate::{domain::eth, infra},
};

mod auction;
mod solution;
mod transaction;
pub use {auction::Auction, solution::Solution, transaction::Transaction};

/// A solution together with the `Auction` for which it was picked as a winner
/// and executed on-chain.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Settlement {
    solution: Solution,
    auction: Auction,
}

impl Settlement {
    pub async fn new(
        tx: &Transaction,
        domain_separator: &eth::DomainSeparator,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let solution = Solution::new(&tx.input, domain_separator)?;
        let auction = persistence.get_auction(solution.auction_id()).await?;

        Ok(Self { solution, auction })
    }

    /// CIP38 score calculation
    pub fn score(&self) -> Result<competition::Score, solution::error::Score> {
        self.solution.score(&self.auction)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Solution(#[from] solution::Error),
    #[error(transparent)]
    Auction(#[from] infra::persistence::error::Auction),
}
