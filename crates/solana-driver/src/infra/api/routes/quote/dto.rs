//! Wire shape of the quote route.

use {
    crate::infra::api::routes::Kind,
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, base64::Base64, serde_as},
    solana_sdk::pubkey::Pubkey,
};

/// The order to quote.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_token: Pubkey,
    /// The sell amount for a sell order, the buy amount for a buy order.
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
    pub kind: Kind,
    /// Absolute deadline by which the solver engine must answer.
    pub deadline: chrono::DateTime<chrono::Utc>,
    /// The wallet the quoted order belongs to, see `transaction`. Absent, no
    /// transaction is answered.
    #[serde_as(as = "Option<DisplayFromStr>")]
    #[serde(default)]
    pub owner: Option<Pubkey>,
}

/// The quoted amounts and what settling them costs.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    #[serde_as(as = "DisplayFromStr")]
    pub sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy_amount: u64,
    /// The solver that produced the quote.
    #[serde_as(as = "DisplayFromStr")]
    pub solver: Pubkey,
    /// The network fee of settling the quoted route, in lamports: the
    /// signature fee plus the priority fee at the solution's compute unit
    /// estimate, the runtime's ceiling when the engine sent none. The amounts
    /// are not reduced by it.
    #[serde_as(as = "DisplayFromStr")]
    pub network_fee: u64,
    /// The settlement of the quoted route, unsigned, bincode-serialized and
    /// base64-encoded: it fills an order of the quoted amounts owned by the
    /// request's `owner`, see `solana_token::quote::QuoteOrder`, for the
    /// orderbook to simulate behind that order's creation. Absent without an
    /// `owner`, or when the settlement could not be built.
    #[serde_as(as = "Option<Base64>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction: Option<Vec<u8>>,
}
