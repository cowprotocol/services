//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use crate::{domain, domain::eth, infra};

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
    pub fn auction_id(&self) -> domain::auction::Id {
        self.solution.auction_id()
    }

    pub async fn new(
        transaction: Transaction,
        domain_separator: &eth::DomainSeparator,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        let solution = Solution::new(&transaction.input, domain_separator)?;

        if persistence
            .auction_has_settlement(solution.auction_id())
            .await
            .map_err(Error::from)?
        {
            // This settlement has already been processed by another environment.
            //
            // TODO: remove once https://github.com/cowprotocol/services/issues/2848 is resolved and ~270 days are passed since bumping.
            return Err(Error::WrongEnvironment);
        }

        let auction = persistence
            .get_auction(solution.auction_id())
            .await
            .map_err(Error::from)?;

        let competition_winner = persistence
            .get_competition_winner(solution.auction_id())
            .await
            .map_err(Error::from)?;

        if transaction.solver != competition_winner.solver() {
            return Err(Error::WinnerMismatch {
                expected: competition_winner.solver(),
                got: transaction.solver,
            });
        }

        // only debugging for now
        match solution.score(&auction) {
            Ok(score) => {
                tracing::debug!(
                    "Settlement for auction {} has score {}",
                    solution.auction_id(),
                    score
                );

                if score < competition_winner.score() {
                    tracing::warn!(
                        "Settlement for auction {} has lower score {} than the competition winner \
                         {}",
                        solution.auction_id(),
                        score,
                        competition_winner.score()
                    );
                }
            }
            Err(err) => {
                tracing::warn!(?err, "failed to calculate score for settlement");
            }
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
    #[error("failed communication with the database: {0}")]
    DatabaseError(sqlx::Error),
    #[error(transparent)]
    Solution(#[from] solution::Error),
    #[error("settlement refers to an auction from a different environment")]
    WrongEnvironment,
    #[error("auction not found in the database")]
    MissingAuction,
    #[error("failed to get fee policy from database: {0} for order: {1}")]
    FeePolicy(infra::persistence::dto::fee_policy::Error, domain::OrderUid),
    #[error("failed to get price from database for token: {0}")]
    Price(eth::TokenAddress),
    #[error("winner mismatch: expected competition winner {expected}, settlement solver {got}")]
    WinnerMismatch {
        expected: eth::Address,
        got: eth::Address,
    },
    #[error("failed to get score from database for a coresponding competition solution, err: {0}")]
    InvalidScore(anyhow::Error),
    #[error(transparent)]
    Score(#[from] solution::error::Score),
    #[error("failed to get competition data from database {0}")]
    SolverCompetition(anyhow::Error),
}

impl From<infra::persistence::error::Auction> for Error {
    fn from(err: infra::persistence::error::Auction) -> Self {
        match err {
            infra::persistence::error::Auction::DatabaseError(err) => Self::DatabaseError(err),
            infra::persistence::error::Auction::Missing => Self::MissingAuction,
            infra::persistence::error::Auction::FeePolicy(err, order) => {
                Self::FeePolicy(err, order)
            }
            infra::persistence::error::Auction::Price(token) => Self::Price(token),
        }
    }
}

impl From<infra::persistence::error::Winner> for Error {
    fn from(err: infra::persistence::error::Winner) -> Self {
        match err {
            infra::persistence::error::Winner::DatabaseError(err) => Self::DatabaseError(err),
            infra::persistence::error::Winner::Missing => Self::MissingAuction,
            infra::persistence::error::Winner::InvalidScore(err) => Self::InvalidScore(err),
            infra::persistence::error::Winner::SolverCompetition(err) => {
                Self::SolverCompetition(err)
            }
        }
    }
}

impl From<infra::persistence::DatabaseError> for Error {
    fn from(err: infra::persistence::DatabaseError) -> Self {
        Self::DatabaseError(err.0)
    }
}
