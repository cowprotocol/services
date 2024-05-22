//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use crate::{domain::eth, domain::auction::order, infra};

mod auction;
mod solution;
mod transaction;
pub use {solution::Solution, transaction::Transaction, auction::Auction};

/// A solution together with the transaction that executed it on-chain.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
pub struct Settlement {
    solution: Solution,
    transaction: Transaction,
}

impl Settlement {
    pub async fn new(tx: eth::TxId, eth: &infra::Ethereum) -> Result<Self, Error> {
        let transaction = eth.transaction(tx).await?;
        let solution = Solution::new(
            &transaction.input.0.clone().into(),
            eth.contracts().settlement_domain_separator(),
        )?;
        Ok(Self {
            solution,
            transaction,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blockchain(#[from] infra::blockchain::Error),
    #[error(transparent)]
    Solution(#[from] solution::Error),
}
