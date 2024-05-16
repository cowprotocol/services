//! This module defines Settlement as originated from a mined transaction
//! calldata.

use crate::{domain::eth, infra};

mod solution;
mod tokenized;
mod trade;
pub use solution::Solution;

/// A transaction that executes a solution. Interacts with the settlement
/// contract `settle` function.
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

/// An on-chain transaction that settled a solution.
#[derive(Debug)]
pub struct Transaction {
    /// The hash of the transaction.
    pub hash: eth::TxId,
    /// The address of the solver that submitted the transaction.
    pub solver: eth::Address,
    /// The call data of the transaction.
    pub input: eth::Calldata,
    /// The block number of the block that contains the transaction.
    pub block: eth::BlockNo,
    /// The gas used by the transaction.
    pub gas: eth::U256,
    /// The effective gas price of the transaction.
    pub effective_gas_price: eth::U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Blockchain(#[from] infra::blockchain::Error),
    #[error(transparent)]
    Solution(#[from] solution::Error),
}
