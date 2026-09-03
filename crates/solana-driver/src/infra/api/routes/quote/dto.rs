//! Wire shape of the quote route.

use {
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
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
}

/// Which amount the order fixes.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Sell,
    Buy,
}

/// The quoted amounts.
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
}

impl From<Kind> for crate::domain::auction::Side {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Sell => Self::Sell,
            Kind::Buy => Self::Buy,
        }
    }
}
