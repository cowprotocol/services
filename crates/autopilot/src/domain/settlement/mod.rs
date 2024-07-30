//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain.

use {
    super::competition,
    crate::{domain::eth, infra},
};

mod auction;
mod jit_order;
mod observation;
mod solution;
mod transaction;
pub use {
    auction::Auction,
    jit_order::JitOrder,
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
        let auction = persistence.get_auction(solution.auction_id()).await?;

        Ok(Self {
            solution,
            transaction,
            auction,
        })
    }

    /// CIP38 score calculation
    pub fn score(&self) -> Result<competition::Score, solution::error::Score> {
        self.solution.score(&self.auction)
    }

    /// Returns the observation of the settlement.
    pub async fn observation(&self, persistence: &infra::Persistence) -> Observation {
        Observation {
            gas: self.transaction.gas,
            gas_price: self.transaction.effective_gas_price,
            surplus: self.solution.native_surplus(&self.auction),
            fee: self.solution.native_fee(&self.auction.prices),
            order_fees: self.solution.fees(),
            jit_orders: self.solution.jit_orders(persistence).await,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Solution(#[from] solution::Error),
    #[error(transparent)]
    Auction(#[from] infra::persistence::error::Auction),
}
