//! Manage solver competition data received by the driver through a private spi.

use {
    anyhow::Result,
    database::auction::AuctionId,
    model::solver_competition::SolverCompetitionAPI,
    primitive_types::H256,
    thiserror::Error,
};

pub enum Identifier {
    Id(AuctionId),
    Transaction(H256),
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
    ) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError>;

    /// Retrieves the solver competition for the most recent auction.
    ///
    /// Returns a `NotFound` error if no solver competition could be found.
    async fn load_latest_competition(
        &self,
    ) -> Result<SolverCompetitionAPI, crate::solver_competition::LoadSolverCompetitionError>;
}

/// Possible errors when loading a solver competition by ID.
#[derive(Debug, Error)]
pub enum LoadSolverCompetitionError {
    #[error("solver competition not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
