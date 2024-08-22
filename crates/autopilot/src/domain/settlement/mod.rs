//! Solvers propose solutions to an [`crate::domain::Auction`].
//!
//! A winning solution becomes a [`Settlement`] once it is executed on-chain in
//! a form of settlement transaction.

use {
    crate::{domain, domain::eth, infra},
    num::Saturating,
    std::collections::HashMap,
};

mod auction;
mod trade;
mod transaction;
pub use {auction::Auction, trade::Trade, transaction::Transaction};

/// A settled transaction together with the `Auction`, for which it was executed
/// on-chain.
///
/// Referenced as a [`Settlement`] in the codebase.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Settlement {
    /// The gas used by the settlement transaction.
    gas: eth::Gas,
    /// The effective gas price of the settlement transaction.
    effective_gas_price: eth::EffectiveGasPrice,
    /// The address of the solver that submitted the settlement transaction.
    solver: eth::Address,
    /// The block number of the block that contains the settlement transaction.
    block: eth::BlockNo,
    /// The associated auction.
    auction: Auction,
    /// Trades that were settled by the transaction.
    trades: Vec<Trade>,
}

impl Settlement {
    /// The gas used by the settlement.
    pub fn gas(&self) -> eth::Gas {
        self.gas
    }

    /// The effective gas price at the time of settlement.
    pub fn gas_price(&self) -> eth::EffectiveGasPrice {
        self.effective_gas_price
    }

    /// Total surplus for all trades in the settlement.
    pub fn surplus_in_ether(&self) -> eth::Ether {
        self.trades
            .iter()
            .map(|trade| {
                trade
                    .surplus_in_ether(&self.auction.prices)
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            ?err,
                            "possible incomplete surplus calculation for trade {}",
                            trade.uid()
                        );
                        num::zero()
                    })
            })
            .sum()
    }

    /// Total fee taken for all the trades in the settlement.
    pub fn fee_in_ether(&self) -> eth::Ether {
        self.trades
            .iter()
            .map(|trade| {
                trade
                    .fee_in_ether(&self.auction.prices)
                    .unwrap_or_else(|err| {
                        tracing::warn!(
                            ?err,
                            "possible incomplete fee calculation for trade {}",
                            trade.uid()
                        );
                        num::zero()
                    })
            })
            .sum()
    }

    /// Per order fees breakdown. Contains all orders from the settlement
    pub fn order_fees(&self) -> HashMap<domain::OrderUid, Option<trade::ExecutedFee>> {
        self.trades
            .iter()
            .map(|trade| {
                (*trade.uid(), {
                    let total = trade.fee_in_sell_token();
                    let protocol = trade.protocol_fees_in_sell_token(&self.auction);
                    match (total, protocol) {
                        (Ok(total), Ok(protocol)) => {
                            let network =
                                total.saturating_sub(protocol.iter().map(|(fee, _)| *fee).sum());
                            Some(trade::ExecutedFee { protocol, network })
                        }
                        _ => None,
                    }
                })
            })
            .collect()
    }

    /// Return all trades that are classified as Just-In-Time (JIT) orders.
    pub fn jit_orders(&self) -> Vec<&trade::Jit> {
        self.trades
            .iter()
            .filter_map(|trade| trade.as_jit())
            .collect()
    }

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
        let database_orders = persistence.orders_that_exist(&settled.order_uids()).await?;

        let trades = settled
            .trades
            .into_iter()
            .map(|trade| {
                // All orders from the auction follow the regular user orders flow
                if auction.orders.contains_key(&trade.uid) {
                    Trade::User(trade::User {
                        uid: trade.uid,
                        sell: trade.sell,
                        buy: trade.buy,
                        side: trade.side,
                        executed: trade.executed,
                        prices: trade.prices,
                    })
                }
                // If not in auction, then check if it's a surplus capturing JIT order
                else if auction
                    .surplus_capturing_jit_order_owners
                    .contains(&trade.uid.owner())
                {
                    Trade::SurplusCapturingJit(trade::Jit {
                        uid: trade.uid,
                        sell: trade.sell,
                        buy: trade.buy,
                        side: trade.side,
                        receiver: trade.receiver,
                        valid_to: trade.valid_to,
                        app_data: trade.app_data,
                        fee_amount: trade.fee_amount,
                        sell_token_balance: trade.sell_token_balance,
                        buy_token_balance: trade.buy_token_balance,
                        signature: trade.signature,
                        executed: trade.executed,
                        prices: trade.prices,
                        created: settled.timestamp,
                    })
                }
                // If not in auction and not a surplus capturing JIT order, then it's a JIT
                // order but it must not be in the database
                else if !database_orders.contains(&trade.uid) {
                    Trade::Jit(trade::Jit {
                        uid: trade.uid,
                        sell: trade.sell,
                        buy: trade.buy,
                        side: trade.side,
                        receiver: trade.receiver,
                        valid_to: trade.valid_to,
                        app_data: trade.app_data,
                        fee_amount: trade.fee_amount,
                        sell_token_balance: trade.sell_token_balance,
                        buy_token_balance: trade.buy_token_balance,
                        signature: trade.signature,
                        executed: trade.executed,
                        prices: trade.prices,
                        created: settled.timestamp,
                    })
                }
                // A regular user order but settled outside of the auction
                else {
                    Trade::UserOutOfAuction(trade::User {
                        uid: trade.uid,
                        sell: trade.sell,
                        buy: trade.buy,
                        side: trade.side,
                        executed: trade.executed,
                        prices: trade.prices,
                    })
                }
            })
            .collect();

        Ok(Self {
            solver: settled.solver,
            block: settled.block,
            gas: settled.gas,
            effective_gas_price: settled.effective_gas_price,
            trades,
            auction,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed communication with the database: {0}")]
    Infra(anyhow::Error),
    #[error("failed to prepare the data fetched from database for domain: {0}")]
    InconsistentData(InconsistentData),
    #[error("settlement refers to an auction from a different environment")]
    WrongEnvironment,
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
            infra::persistence::error::Auction::DatabaseError(err) => Self::Infra(err.into()),
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
            infra::persistence::error::Solution::DatabaseError(err) => Self::Infra(err.into()),
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
