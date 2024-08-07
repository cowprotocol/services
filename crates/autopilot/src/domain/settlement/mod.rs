//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use {
    super::competition,
    crate::{domain, domain::eth, infra},
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
    ) -> Result<Self, ErrorWithAuction> {
        let solution = Solution::new(&transaction.input, domain_separator)?;
        let auction_id = solution.auction_id();

        if persistence
            .auction_has_settlement(auction_id)
            .await
            .map_err(with(auction_id))?
        {
            // This settlement has already been processed by another environment.
            //
            // TODO: remove once https://github.com/cowprotocol/services/issues/2848 is resolved and ~270 days are passed since bumping.
            return Err(Error::WrongEnvironment).map_err(with(auction_id));
        }

        let auction = persistence
            .get_auction(auction_id)
            .await
            .map_err(with(auction_id))?;

        // winning solution - solution promised during solver competition
        let promised_solution = persistence
            .get_winning_solution(auction_id)
            .await
            .map_err(with(auction_id))?;

        if transaction.solver != promised_solution.solver() {
            return Err(Error::SolverMismatch {
                expected: promised_solution.solver(),
                got: transaction.solver,
            })
            .map_err(with(auction_id));
        }

        let score = solution.score(&auction).map_err(with(auction_id))?;

        // temp log
        if score != promised_solution.score() {
            tracing::debug!(
                ?auction_id,
                "score mismatch: expected competition score {}, settlement score {}",
                promised_solution.score(),
                score,
            );
        }
        if score < promised_solution.score() {
            return Err(Error::ScoreMismatch {
                expected: promised_solution.score(),
                got: score,
            })
            .map_err(with(auction_id));
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
            order_fees: self.solution.fees(&self.auction.prices),
        }
    }
}

#[derive(Debug)]
pub struct ErrorWithAuction {
    #[allow(dead_code)]
    pub inner: Error,
    pub auction_id: Option<domain::auction::Id>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("calldata is not a settlement transaction")]
    NotSettlement,
    #[error("auction id not attached to the calldata")]
    MissingAuctionId,
    #[error("failed to decode settlement: {0}")]
    Decoding(#[source] solution::error::Decoding),
    #[error("failed communication with the database: {0}")]
    Infra(sqlx::Error),
    #[error("failed to prepare the data fetched from database for domain: {0}")]
    InconsistentData(InconsistentData),
    #[error("settlement refers to an auction from a different environment")]
    WrongEnvironment,
    #[error(transparent)]
    BuildingScore(#[from] solution::error::Score),
    #[error("solver mismatch: expected competition solver {expected}, settlement solver {got}")]
    SolverMismatch {
        expected: eth::Address,
        got: eth::Address,
    },
    #[error("score mismatch: expected competition score {expected}, settlement score {got}")]
    ScoreMismatch {
        expected: competition::Score,
        got: competition::Score,
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

impl From<solution::Error> for ErrorWithAuction {
    fn from(err: solution::Error) -> Self {
        match err {
            solution::Error::NotSettlement => ErrorWithAuction {
                inner: Error::NotSettlement,
                auction_id: None,
            },
            solution::Error::MissingAuctionId => ErrorWithAuction {
                inner: Error::MissingAuctionId,
                auction_id: None,
            },
            solution::Error::Decoding(err, auction_id) => ErrorWithAuction {
                inner: Error::Decoding(err),
                auction_id: Some(auction_id),
            },
        }
    }
}

fn with<E>(auction: domain::auction::Id) -> impl FnOnce(E) -> ErrorWithAuction
where
    E: Into<Error>,
{
    move |err| {
        let err = err.into();
        ErrorWithAuction {
            inner: err,
            auction_id: Some(auction),
        }
    }
}
