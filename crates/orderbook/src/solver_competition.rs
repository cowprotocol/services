//! Manage solver competition data received by the driver through a private spi.

use anyhow::Result;
use database::auction::AuctionId;
use model::{order::OrderUid, solver_competition::SolverCompetition};
use primitive_types::H256;
use std::collections::HashMap;
use thiserror::Error;

pub enum Identifier {
    Id(AuctionId),
    Transaction(H256),
}

/// Component used for saving and loading past solver competitions.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait SolverCompetitionStoring: Send + Sync {
    /// Saves a new solver competition entry.
    async fn save_competition(&self, competition: SolverCompetition) -> Result<()>;

    /// Saves the risk adjusted solver rewards.
    async fn save_rewards(&self, auction: AuctionId, rewards: HashMap<OrderUid, f64>)
        -> Result<()>;

    /// Retrieves a solver competition entry by ID.
    ///
    /// Returns a `NotFound` error if no solver competition with that ID could
    /// be found.
    async fn load_competition(
        &self,
        identifier: Identifier,
    ) -> Result<SolverCompetition, LoadSolverCompetitionError>;
}

/// Possible errors when loading a solver competition by ID.
#[derive(Debug, Error)]
pub enum LoadSolverCompetitionError {
    #[error("solver competition not found")]
    NotFound,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
