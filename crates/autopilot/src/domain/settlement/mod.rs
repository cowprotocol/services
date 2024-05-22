//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use crate::{domain::eth, infra};

mod auction;
mod solution;
mod transaction;
pub use {auction::Auction, solution::Solution, transaction::Transaction};

/// A solution together with the transaction that executed it on-chain.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Settlement {
    solution: Solution,
    transaction: Transaction,
    auction: Auction,
}

impl Settlement {
    pub async fn new(
        tx: eth::TxId,
        eth: &infra::Ethereum,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let transaction = eth.transaction(tx).await?;
        let solution = Solution::new(
            &transaction.input.0.clone().into(),
            eth.contracts().settlement_domain_separator(),
        )?;
        let auction = persistence.get_settlement_auction(&solution).await?;
        Ok(Self {
            solution,
            transaction,
            auction,
        })
    }

    /// The onchain delivered score of a solution.
    pub fn score(&self) -> Result<super::competition::Score, solution::error::Score> {
        self.solution
            .score(&self.auction.prices, &self.auction.fee_policies)
    }

    /// The competition score, which is the score promised by solver, during the
    /// competition.
    pub fn competition_score(&self) -> super::competition::Score {
        self.auction.score
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blockchain(#[from] infra::blockchain::Error),
    #[error(transparent)]
    Solution(#[from] solution::Error),
    #[error(transparent)]
    Auction(#[from] infra::persistence::error::Auction),
}
