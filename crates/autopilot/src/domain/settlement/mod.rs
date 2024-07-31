//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use {
    super::competition,
    crate::{domain::eth, infra},
};

mod auction;
mod observation;
mod solution;
mod transaction;
pub use {
    auction::Auction,
    observation::Observation,
    solution::Solution,
    transaction::Transaction,
};

/// A solution together with the `Auction` for which it was picked as a winner
/// and executed on-chain.
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
        transaction: Transaction,
        domain_separator: &eth::DomainSeparator,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let solution = Solution::new(&transaction.input, domain_separator)?;

        if persistence
            .auction_has_settlement(solution.auction_id())
            .await?
        {
            // This settlement has already been processed by another environment.
            //
            // TODO: remove once https://github.com/cowprotocol/services/issues/2848 is resolved and ~270 days are passed since bumping.
            return Err(Error::WrongEnvironment);
        }

        let auction = persistence.get_auction(solution.auction_id()).await?;
        let (winner, winner_score, deadline) =
            persistence.get_winner(solution.auction_id()).await?;

        if transaction.solver != winner {
            return Err(Error::WinnerMismatch {
                expected: winner,
                got: transaction.solver,
            });
        }

        let score = solution.score(&auction)?;
        if score != winner_score {
            return Err(Error::ScoreMismatch {
                expected: winner_score,
                got: score,
            });
        }

        if transaction.block >= deadline {
            tracing::warn!(
                "Settlement for auction {} was submitted after the deadline",
                solution.auction_id()
            );
        }

        Ok(Self {
            solution,
            transaction,
            auction,
        })
    }

    /// Returns the observation of the settlement.
    pub fn observation(&self) -> Observation {
        Observation {
            gas: self.transaction.gas,
            gas_price: self.transaction.effective_gas_price,
            surplus: self.solution.native_surplus(&self.auction),
            fee: self.solution.native_fee(&self.auction.prices),
            order_fees: self.solution.fees(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Solution(#[from] solution::Error),
    #[error("settlement refers to an auction from a different environment")]
    WrongEnvironment,
    #[error(transparent)]
    PersistenceConnection(#[from] infra::persistence::Error),
    #[error(transparent)]
    Auction(#[from] infra::persistence::error::Auction),
    #[error(transparent)]
    Winner(#[from] infra::persistence::error::Winner),
    #[error("winner mismatch: expected {expected}, got {got}")]
    WinnerMismatch {
        expected: eth::Address,
        got: eth::Address,
    },
    #[error("score mismatch: expected {expected}, got {got}")]
    ScoreMismatch {
        expected: competition::Score,
        got: competition::Score,
    },
    #[error(transparent)]
    Score(#[from] solution::error::Score),
}
