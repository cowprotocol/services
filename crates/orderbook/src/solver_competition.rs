//! Manage solver competition data received by the driver through a private spi.

use {
    alloy::primitives::B256,
    anyhow::Result,
    database::auction::AuctionId,
    model::solver_competition::SolverCompetitionAPI,
    thiserror::Error,
};

pub enum Identifier {
    Id(AuctionId),
    Transaction(B256),
}

/// Component used for saving and loading past solver competitions.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait SolverCompetitionStoring: Send + Sync {
    /// Retrieves a solver competition entry by ID.
    ///
    /// Returns a `NotFound` error if no solver competition with that ID could
    /// be found.
    async fn load_competition(
        &self,
        identifier: Identifier,
    ) -> Result<SolverCompetitionAPI, crate::solver_competition::LoadSolverCompetitionError>;

    /// Retrieves the solver competition for the most recent auction.
    ///
    /// Returns a `NotFound` error if no solver competition could be found.
    async fn load_latest_competition(
        &self,
    ) -> Result<SolverCompetitionAPI, crate::solver_competition::LoadSolverCompetitionError>;

    /// Retrieves the solver competitions for the most recent auctions.
    ///
    /// Returns the latest solver competitions.
    /// It may return fewer results than specified by
    /// `latest_competitions_count` if not enough solver competitions are
    /// found.
    async fn load_latest_competitions(
        &self,
        latest_competitions_count: u32,
    ) -> Result<Vec<SolverCompetitionAPI>, crate::solver_competition::LoadSolverCompetitionError>;
}

/// Possible errors when loading a solver competition by ID.
#[derive(Debug, Error)]
pub enum LoadSolverCompetitionError {
    #[error("solver competition not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
