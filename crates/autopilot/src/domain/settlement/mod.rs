//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use crate::{domain::eth, infra};

mod auction;
mod solution;
mod trade;
mod transaction;
pub use {
    auction::Auction,
    trade::{tokenized, Trade},
    transaction::Transaction,
};

/// A solution together with the `Auction` for which it was picked as a winner.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Settlement {
    solution: solution::Solution,
    auction: Auction,
}

impl Settlement {
    pub async fn new(
        tx: &Transaction,
        domain_separator: &eth::DomainSeparator,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let solution = solution::Solution::new(tx, domain_separator, persistence).await?;
        let auction = persistence.get_auction(solution.auction_id()).await?;

        Ok(Self { solution, auction })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Solution(#[from] solution::Error),
    #[error(transparent)]
    Auction(#[from] infra::persistence::error::Auction),
}
