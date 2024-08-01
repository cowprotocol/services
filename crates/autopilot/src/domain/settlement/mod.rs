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

        let winning_solution = persistence
            .get_winning_solution(solution.auction_id())
            .await
            .map_err(Error::from)?;

        if transaction.solver != winning_solution.solver() {
            return Err(Error::SolverMismatch {
                expected: winning_solution.solver(),
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

                if score < winning_solution.score() {
                    tracing::warn!(
                        "Settlement for auction {} has lower score {} than the winning solution {}",
                        solution.auction_id(),
                        score,
                        winning_solution.score()
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "Settlement for auction {} failed to calculate score",
                    solution.auction_id()
                );
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
    BadCommunication(sqlx::Error),
    #[error("failed to prepare the data fetched from database for domain: {0}")]
    BadPersistenceData(BadPersistenceData),
    #[error("settlement refers to an auction from a different environment")]
    WrongEnvironment,
    #[error(transparent)]
    BuildingSolution(#[from] solution::Error),
    #[error(transparent)]
    BuildingScore(#[from] solution::error::Score),
    #[error("solver mismatch: expected competition solver {expected}, settlement solver {got}")]
    SolverMismatch {
        expected: eth::Address,
        got: eth::Address,
    },
}

/// Errors that can occur when fetching data from the persistence layer.
///
/// These errors cover missing data, conversion of data into domain objects etc.
///
/// This is a separate enum to allow for more specific error handling.
#[derive(Debug, thiserror::Error)]
pub enum BadPersistenceData {
    #[error("auction not found in the persistence layer")]
    AuctionNotFound,
    #[error("proposed solution not found in the persistence layer")]
    SolutionNotFound,
    #[error("invalid fee policy fetched from persistence layer: {0} for order: {1}")]
    InvalidFeePolicy(infra::persistence::dto::fee_policy::Error, domain::OrderUid),
    #[error("invalid fetched price from persistence layer for token: {0}")]
    InvalidPricce(eth::TokenAddress),
    #[error(
        "invalid score fetched from persistence layer for a coresponding competition solution, \
         err: {0}"
    )]
    InvalidScore(anyhow::Error),
    #[error("invalid solver competition data fetched from persistence layer: {0}")]
    InvalidSolverCompetition(anyhow::Error),
}

impl From<infra::persistence::error::Auction> for Error {
    fn from(err: infra::persistence::error::Auction) -> Self {
        match err {
            infra::persistence::error::Auction::BadCommunication(err) => {
                Self::BadCommunication(err)
            }
            infra::persistence::error::Auction::NotFound => {
                Self::BadPersistenceData(BadPersistenceData::AuctionNotFound)
            }
            infra::persistence::error::Auction::InvalidFeePolicy(err, order) => {
                Self::BadPersistenceData(BadPersistenceData::InvalidFeePolicy(err, order))
            }
            infra::persistence::error::Auction::InvalidPrice(token) => {
                Self::BadPersistenceData(BadPersistenceData::InvalidPricce(token))
            }
        }
    }
}

impl From<infra::persistence::error::Solution> for Error {
    fn from(err: infra::persistence::error::Solution) -> Self {
        match err {
            infra::persistence::error::Solution::BadCommunication(err) => {
                Self::BadCommunication(err)
            }
            infra::persistence::error::Solution::NotFound => {
                Self::BadPersistenceData(BadPersistenceData::SolutionNotFound)
            }
            infra::persistence::error::Solution::InvalidScore(err) => {
                Self::BadPersistenceData(BadPersistenceData::InvalidScore(err))
            }
            infra::persistence::error::Solution::InvalidSolverCompetition(err) => {
                Self::BadPersistenceData(BadPersistenceData::InvalidSolverCompetition(err))
            }
            infra::persistence::error::Solution::InvalidPrice(_) => todo!(),
        }
    }
}

impl From<infra::persistence::DatabaseError> for Error {
    fn from(err: infra::persistence::DatabaseError) -> Self {
        Self::BadCommunication(err.0)
    }
}
