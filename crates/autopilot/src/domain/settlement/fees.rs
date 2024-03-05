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

// TODO

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
                let fee = trade.fee_in_sell_token().unwrap_or_else(|| {
                    tracing::warn!("fee failed for trade {:?}", trade.order_uid);
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

    // pub fn normalized_with(
    //     &self,
    //     prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    // ) -> Option<NormalizedFee> {
    //     let mut fees = eth::TokenAmount::default();
    //     for eth::Asset { token, amount } in self.0.values() {
    //         let price = prices.get(token).cloned()?;
    //         let amount: eth::SimpleValue<BigRational> =
    // amount.to_big_rational().into();         let normalized_fee =
    // big_rational_to_u256(&(amount * price)).ok()?.into();         fees +=
    // normalized_fee;     }
    //     Some(fees)
    // }
}

/// Normalized fee
///
/// Denominated in the native token (ETH)
pub type NormalizedFee = eth::TokenAmount; // eth::Ether?
