//! Manage solver competition data received by the driver through a private spi.

use thiserror::Error;

/// Possible errors when loading a solver competition by ID.
#[derive(Debug, Error)]
pub enum LoadSolverCompetitionError {
    #[error("solver competition not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
