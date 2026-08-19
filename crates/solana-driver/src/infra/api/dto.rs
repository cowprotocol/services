//! Wire DTOs for the autopilot-facing `/solve` and `/settle` endpoints.
//!
//! Mirrors the autopilot's driver client
//! (`autopilot-svm/src/infra/driver/dto.rs`): camelCase, pubkeys as base58,
//! order uids as `0x`-hex, u64 token amounts as decimal strings.
//!
//! TODO: predicted shape. The real auction pre-processing decides what of
//! this survives, until then the fields mirror the autopilot side verbatim.

use {
    crate::domain::order_uid::OrderUid,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::{pubkey::Pubkey, signature::Signature},
    std::collections::HashMap,
};

/// The auction the autopilot posts to `/solve`.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    pub id: i64,
    /// Slot after which a settlement for this auction is late.
    pub deadline_slot: u64,
    pub orders: Vec<Order>,
}

/// One solvable order.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "DisplayFromStr")]
    pub uid: OrderUid,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Sell,
    Buy,
}

/// The `/solve` answer.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveResponse {
    pub solutions: Vec<Solution>,
}

/// One proposed solution.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    pub solution_id: u64,
    /// Total surplus in lamports, decimal string on the wire.
    /// TODO: always zero, the autopilot recomputes scores itself. Real
    /// scoring arrives with trade and fee math.
    #[serde_as(as = "DisplayFromStr")]
    pub score: u64,
    /// The keypair the driver settles with, the on-chain solver identity.
    #[serde_as(as = "DisplayFromStr")]
    pub solver: Pubkey,
    /// Executed amounts per filled order.
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub orders: HashMap<OrderUid, TradedAmounts>,
}

/// What a solution executes for one order.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradedAmounts {
    #[serde_as(as = "DisplayFromStr")]
    pub executed_sell: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub executed_buy: u64,
}

/// Asks the driver to submit a previously proposed solution.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    pub auction_id: i64,
    pub solution_id: u64,
}

/// The `/settle` answer.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    /// Transaction signature of the submitted settlement.
    #[serde_as(as = "DisplayFromStr")]
    pub tx_signature: Signature,
}
