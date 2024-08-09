//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use {
    crate::{domain, domain::eth, infra},
    std::collections::HashMap,
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
    settled: Transaction,
    auction: Auction,
}

impl Settlement {
    pub async fn new(
        settled: Transaction,
        persistence: &infra::Persistence,
    ) -> Result<Self, Error> {
        if persistence
            .auction_has_settlement(settled.auction_id)
            .await?
        {
            // This settlement has already been processed by another environment.
            //
            // TODO: remove once https://github.com/cowprotocol/services/issues/2848 is resolved and ~270 days are passed since bumping.
            return Err(Error::WrongEnvironment);
        }

        let auction = persistence.get_auction(settled.auction_id).await?;

        // winning solution - solution promised during solver competition
        let promised_solution = persistence.get_winning_solution(settled.auction_id).await?;

        if settled.solver != promised_solution.solver() {
            return Err(Error::SolverMismatch {
                expected: promised_solution.solver(),
                got: settled.solver,
            });
        }

        let score = settled.solution.score(&auction)?;

        // temp log
        if score != promised_solution.score() {
            tracing::debug!(
                ?settled.auction_id,
                "score mismatch: expected competition score {}, settlement score {}",
                promised_solution.score(),
                score,
            );
        }

        Ok(Self { settled, auction })
    }

    /// The gas used by the settlement.
    pub fn gas(&self) -> eth::Gas {
        self.settled.gas
    }

    /// The effective gas price at the time of settlement.
    pub fn gas_price(&self) -> eth::EffectiveGasPrice {
        self.settled.effective_gas_price
    }

    /// Total surplus expressed in native token.
    pub fn native_surplus(&self) -> eth::Ether {
        self.settled.solution.native_surplus(&self.auction)
    }

    /// Total fee expressed in native token.
    pub fn native_fee(&self) -> eth::Ether {
        self.settled.solution.native_fee(&self.auction.prices)
    }

    /// Per order fees denominated in sell token. Contains all orders from the
    /// settlement
    pub fn order_fees(&self) -> HashMap<domain::OrderUid, Option<eth::SellTokenAmount>> {
        self.settled.solution.fees(&self.auction.prices)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed communication with the database: {0}")]
    Infra(sqlx::Error),
    #[error("failed to prepare the data fetched from database for domain: {0}")]
    InconsistentData(InconsistentData),
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
pub enum InconsistentData {
    #[error("auction not found in the persistence layer")]
    AuctionNotFound,
    #[error("proposed solution not found in the persistence layer")]
    SolutionNotFound,
    #[error("invalid fee policy fetched from persistence layer: {0} for order: {1}")]
    InvalidFeePolicy(infra::persistence::dto::fee_policy::Error, domain::OrderUid),
    #[error("invalid fetched price from persistence layer for token: {0:?}")]
    InvalidPrice(eth::TokenAddress),
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
            infra::persistence::error::Auction::BadCommunication(err) => Self::Infra(err),
            infra::persistence::error::Auction::NotFound => {
                Self::InconsistentData(InconsistentData::AuctionNotFound)
            }
            infra::persistence::error::Auction::InvalidFeePolicy(err, order) => {
                Self::InconsistentData(InconsistentData::InvalidFeePolicy(err, order))
            }
            infra::persistence::error::Auction::InvalidPrice(token) => {
                Self::InconsistentData(InconsistentData::InvalidPrice(token))
            }
        }
    }
}

impl From<infra::persistence::error::Solution> for Error {
    fn from(err: infra::persistence::error::Solution) -> Self {
        match err {
            infra::persistence::error::Solution::BadCommunication(err) => Self::Infra(err),
            infra::persistence::error::Solution::NotFound => {
                Self::InconsistentData(InconsistentData::SolutionNotFound)
            }
            infra::persistence::error::Solution::InvalidScore(err) => {
                Self::InconsistentData(InconsistentData::InvalidScore(err))
            }
            infra::persistence::error::Solution::InvalidSolverCompetition(err) => {
                Self::InconsistentData(InconsistentData::InvalidSolverCompetition(err))
            }
            infra::persistence::error::Solution::InvalidPrice(_) => todo!(),
        }
    }
}

impl From<infra::persistence::DatabaseError> for Error {
    fn from(err: infra::persistence::DatabaseError) -> Self {
        Self::Infra(err.0)
    }
}
