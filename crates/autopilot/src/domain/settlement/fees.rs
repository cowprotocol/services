//! There are two types of fees in the system: gas cost fees and protocol fees.
//!
//! Gas cost fees are fees that are paid to a network for the onchain
//! execution of the settlement. These fees are paid in ETH and are calculated
//! based on the amount of gas used by the transaction. The gas cost fees are
//! determined by solvers.
//!
//! Protocol fees are fees that are paid to the protocol for it's services.
//! These fees are paid in the native token of the protocol and are determined
//! by the protocol.

use {
    crate::domain::{auction, eth, settlement},
    std::collections::HashMap,
};

/// Observable fee based on the mined settlement.
///
/// The difference between the uniform and custom prices is the fee.
///
/// Expressed in the SELL token.

#[derive(Debug, Clone)]
pub struct Fees(HashMap<auction::order::OrderUid, eth::Asset>);

impl Fees {
    pub fn new(trades: &[settlement::Trade]) -> Self {
        let fees = trades
            .iter()
            .map(|trade| {
                let fee = trade.fee_in_sell_token().unwrap_or_else(|err| {
                    tracing::warn!("fee failed for trade {:?}, err {}", trade.order_uid, err);
                    eth::Asset {
                        token: trade.sell.token,
                        amount: Default::default(),
                    }
                });
                (trade.order_uid, fee)
            })
            .collect();
        Self(fees)
    }

    pub fn get(&self) -> &HashMap<auction::order::OrderUid, eth::Asset> {
        &self.0
    }
}
