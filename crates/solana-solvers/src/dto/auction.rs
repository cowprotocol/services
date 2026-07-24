//! Inbound `/solve` auction: the orders the driver asks the solver to fill.
//!
//! Proposed shape. The driver spec pins the solution response, not the auction
//! request, so these are the fields the solve loop needs and will be reconciled
//! with the driver's request DTO.

use {
    super::order::OrderUid,
    crate::dex,
    serde::Deserialize,
    serde_with::serde_as,
    solana_sdk::pubkey::Pubkey,
};

/// The auction the driver posts to `/solve`.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: u64,
    /// Settlement signer the swap instructions are built for.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub taker: Pubkey,
    pub orders: Vec<Order>,
}

/// One order to quote.
#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub uid: OrderUid,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub sell_mint: Pubkey,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_mint: Pubkey,
    /// The buy-mint buffer the swap output lands in.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_destination: Pubkey,
    /// Sell amount for a sell, buy amount for a buy. Decimal string on the
    /// wire.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub amount: u64,
    pub side: dex::Side,
}

impl Order {
    /// The adapter-facing view of this order.
    pub fn to_dex_order(&self) -> dex::Order {
        dex::Order {
            sell_mint: self.sell_mint,
            buy_mint: self.buy_mint,
            buy_destination: self.buy_destination,
            amount: self.amount,
            side: self.side,
        }
    }
}
