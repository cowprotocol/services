//! Wire DTOs for the driver's `/solve` and `/settle` endpoints.
//!
//! Conventions shared with the solver engine API: camelCase fields, pubkeys
//! as base58 strings, order uids as `0x`-hex, token amounts as decimal
//! strings (u64 does not survive JSON number consumers).

use {
    crate::domain::auction,
    chain_types::solana::{IntentHash, Pubkey},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
};

/// The auction posted to `/solve`.
#[serde_as]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    /// Autopilot-assigned auction id.
    pub id: i64,
    /// Slot after which a settlement for this auction is late.
    pub deadline_slot: u64,
    pub orders: Vec<Order>,
}

/// One solvable order in the auction.
#[serde_as]
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResponse {
    pub solutions: Vec<Solution>,
}

/// One proposed solution, the ranking input.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub solution_id: u64,
    /// Total surplus in lamports, decimal string on the wire.
    #[serde_as(as = "DisplayFromStr")]
    pub score: u64,
}

/// Asks the driver to submit a previously proposed solution.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    pub auction_id: i64,
    pub solution_id: u64,
}

/// The driver's `/settle` answer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    /// Base58 transaction signature of the submitted settlement.
    pub tx_signature: String,
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

    #[test]
    fn solve_request_serializes_the_wire_conventions() {
        let request = SolveRequest {
            id: 7,
            deadline_slot: 100,
            orders: vec![Order::from(&order())],
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["id"], 7);
        assert_eq!(json["deadlineSlot"], 100);
        let order = &json["orders"][0];
        assert_eq!(
            order["uid"],
            "0x1111111111111111111111111111111111111111111111111111111111111111"
        );
        // u64::MAX survives as a decimal string.
        assert_eq!(order["sellAmount"], "18446744073709551615");
        assert_eq!(order["kind"], "sell");
        // Base58 of 32 bytes of 0x22.
        assert_eq!(
            order["owner"],
            chain_types::solana::Pubkey([0x22; 32]).to_string()
        );
    }

    #[test]
    fn responses_deserialize() {
        let solve: SolveResponse =
            serde_json::from_str(r#"{"solutions":[{"solutionId":3,"score":"12345"}]}"#).unwrap();
        assert_eq!(solve.solutions[0].solution_id, 3);
        assert_eq!(solve.solutions[0].score, 12_345);

        let settle: SettleResponse = serde_json::from_str(r#"{"txSignature":"5bpPk"}"#).unwrap();
        assert_eq!(settle.tx_signature, "5bpPk");
    }
}
