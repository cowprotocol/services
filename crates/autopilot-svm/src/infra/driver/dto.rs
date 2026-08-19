//! Wire DTOs for the driver's `/solve` and `/settle` endpoints.
//!
//! Conventions shared with the solver engine API: camelCase fields, pubkeys
//! as base58 strings, order uids as `0x`-hex, token amounts as decimal
//! strings (u64 does not survive JSON number consumers).

use {
    crate::domain::auction,
    chain_types::solana::{IntentHash, Pubkey, Signature},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    std::collections::HashMap,
};

/// The auction posted to `/solve`.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    /// Autopilot-assigned auction id.
    #[serde_as(as = "DisplayFromStr")]
    pub id: i64,
    /// Slot after which a settlement for this auction is late.
    #[serde_as(as = "DisplayFromStr")]
    pub deadline_slot: u64,
    pub orders: Vec<Order>,
}

/// One solvable order in the auction.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "DisplayFromStr")]
    pub uid: IntentHash,
    #[serde_as(as = "DisplayFromStr")]
    pub owner: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: u64,
    /// Unix seconds.
    pub valid_to: u32,
    pub kind: Kind,
    pub partially_fillable: bool,
    #[serde_as(as = "DisplayFromStr")]
    pub order_pda: Pubkey,
}

/// Whether the order sells or buys an exact amount.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Sell,
    Buy,
}

impl From<&auction::Order> for Order {
    fn from(order: &auction::Order) -> Self {
        Self {
            uid: order.uid,
            owner: order.owner,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_token_account: order.sell_token_account,
            buy_token_account: order.buy_token_account,
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            valid_to: order.valid_to,
            kind: match order.kind {
                auction::OrderKind::Sell => Kind::Sell,
                auction::OrderKind::Buy => Kind::Buy,
            },
            partially_fillable: order.partially_fillable,
            order_pda: order.order_pda,
        }
    }
}

/// The driver's `/solve` answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResponse {
    pub solutions: Vec<Solution>,
}

/// One proposed solution: enough to rank it, detect order overlap between
/// winners, and attribute its settlement on chain.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    #[serde_as(as = "DisplayFromStr")]
    pub solution_id: u64,
    /// Total surplus in lamports, decimal string on the wire.
    #[serde_as(as = "DisplayFromStr")]
    pub score: u64,
    /// The keypair the driver settles with, the on-chain solver identity.
    #[serde_as(as = "DisplayFromStr")]
    pub solver: Pubkey,
    /// Executed amounts per filled order.
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub orders: HashMap<IntentHash, TradedAmounts>,
}

/// What a solution executes for one order.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedAmounts {
    #[serde_as(as = "DisplayFromStr")]
    pub executed_sell: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub executed_buy: u64,
}

/// Asks the driver to submit a previously proposed solution.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub auction_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub solution_id: u64,
}

/// The driver's `/settle` answer.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    /// Transaction signature of the submitted settlement.
    #[serde_as(as = "DisplayFromStr")]
    pub tx_signature: Signature,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> auction::Order {
        auction::Order {
            uid: IntentHash([0x11; 32]),
            owner: Pubkey([0x22; 32]),
            sell_token: Pubkey([0x33; 32]),
            buy_token: Pubkey([0x44; 32]),
            sell_token_account: Pubkey([0x55; 32]),
            buy_token_account: Pubkey([0x66; 32]),
            sell_amount: u64::MAX,
            buy_amount: 2_000,
            valid_to: 42,
            kind: auction::OrderKind::Sell,
            partially_fillable: false,
            order_pda: Pubkey([0x77; 32]),
        }
    }

    /// One pass pins the wire conventions with literals (a pure round trip
    /// is self-consistent even when the format is wrong) and round-trips
    /// every DTO through serde.
    #[test]
    fn dtos_round_trip_and_pin_the_wire_format() {
        let request = SolveRequest {
            id: 7,
            deadline_slot: 100,
            orders: vec![Order::from(&order())],
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["id"], "7");
        assert_eq!(json["deadlineSlot"], "100");
        assert_eq!(
            json["orders"][0]["uid"],
            "0x1111111111111111111111111111111111111111111111111111111111111111"
        );
        // u64::MAX survives as a decimal string.
        assert_eq!(json["orders"][0]["sellAmount"], "18446744073709551615");
        assert_eq!(json["orders"][0]["kind"], "sell");
        // Base58 of 32 bytes of 0x22, precomputed so this does not just
        // compare the Display impl against itself.
        assert_eq!(
            json["orders"][0]["owner"],
            "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3"
        );
        assert_eq!(
            serde_json::from_value::<SolveRequest>(json).unwrap(),
            request
        );

        let solve = SolveResponse {
            solutions: vec![Solution {
                solution_id: 3,
                score: 12_345,
                solver: Pubkey([0x22; 32]),
                orders: HashMap::from([(
                    IntentHash([0x11; 32]),
                    TradedAmounts {
                        executed_sell: 100,
                        executed_buy: 200,
                    },
                )]),
            }],
        };
        let json = serde_json::to_value(&solve).unwrap();
        assert_eq!(json["solutions"][0]["solutionId"], "3");
        assert_eq!(json["solutions"][0]["score"], "12345");
        assert_eq!(
            serde_json::from_value::<SolveResponse>(json).unwrap(),
            solve
        );

        let settle = SettleResponse {
            tx_signature: Signature([9; 64]),
        };
        let json = serde_json::to_value(&settle).unwrap();
        assert_eq!(
            serde_json::from_value::<SettleResponse>(json).unwrap(),
            settle
        );

        let settle_request = SettleRequest {
            auction_id: 7,
            solution_id: 3,
        };
        let json = serde_json::to_value(&settle_request).unwrap();
        assert_eq!(json["auctionId"], "7");
        assert_eq!(json["solutionId"], "3");
        assert_eq!(
            serde_json::from_value::<SettleRequest>(json).unwrap(),
            settle_request
        );
    }
}
