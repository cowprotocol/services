//! Inbound `/solve` auction: the orders the driver asks the solver to fill.
//!
//! The wire format matches `solana-driver/src/infra/solver/dto/auction.rs`,
//! whose `wire_format_is_stable` test pins it.

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
    /// `None` when the auction prices a quote instead of a competition.
    pub id: Option<i64>,
    /// Settlement signer the swap instructions are built for.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub taker: Pubkey,
    pub orders: Vec<Order>,
    /// Absolute deadline by which solutions must be returned.
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// One order to quote.
///
/// The limit leg may arrive tightened by the driver's solver fee. Fills are
/// checked against the tightened legs. The signed amounts are carried for
/// external engines; a fill checked against them can be pushed under the
/// limit by the fee.
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
    /// Sell-mint units: for a sell order the amount to fill, for a buy order
    /// the most the fill may take. Decimal string on the wire.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub sell_amount: u64,
    /// Buy-mint units: for a buy order the amount to fill, for a sell order
    /// the least the fill must deliver. Decimal string on the wire.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_amount: u64,
    /// Sell amount for a sell, buy amount for a buy. Decimal string on the
    /// wire.
    ///
    /// TODO: remove once external engines read `sellAmount`/`buyAmount`.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub amount: u64,
    /// The signed sell amount, before any solver-fee tightening. Decimal
    /// string on the wire.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub full_sell_amount: u64,
    /// The signed buy amount, before any solver-fee tightening. Decimal string
    /// on the wire.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub full_buy_amount: u64,
    pub side: dex::Side,
    /// True when the order's buy token account does not exist on chain
    /// yet. The driver's settlement creates it and the solver keypair pays
    /// its rent, a cost the solution should price in.
    ///
    /// TODO(token-2022): a token-2022 account rents more bytes, so once
    /// those mints are supported this boolean becomes the missing account's
    /// token program.
    #[serde(default)]
    pub missing_buy_token_account: bool,
}

impl Order {
    /// The adapter-facing view of this order.
    pub fn to_dex_order(&self) -> dex::Order {
        dex::Order {
            sell_mint: self.sell_mint,
            buy_mint: self.buy_mint,
            buy_destination: self.buy_destination,
            sell_amount: self.sell_amount,
            buy_amount: self.buy_amount,
            side: self.side,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    /// The literal `solana-driver`'s `wire_format_is_stable` test serializes.
    #[test]
    fn parses_the_driver_request() {
        let json = serde_json::json!({
            "id": 1,
            "taker": pubkey(3).to_string(),
            "orders": [{
                "uid": format!("0x{}", "08".repeat(32)),
                "sellMint": pubkey(1).to_string(),
                "buyMint": pubkey(2).to_string(),
                "buyDestination": pubkey(4).to_string(),
                "sellAmount": "1000",
                "buyAmount": "2000",
                "amount": "1000",
                "fullSellAmount": "1000",
                "fullBuyAmount": "2000",
                "side": "sell",
            }],
            "deadline": "2026-01-01T00:00:00Z",
        });

        let auction: Auction = serde_json::from_value(json).unwrap();

        assert_eq!(auction.id, Some(1));
        assert_eq!(auction.taker, pubkey(3));
        let order = &auction.orders[0];
        assert_eq!(order.uid, OrderUid([8; 32]));
        assert_eq!((order.sell_mint, order.buy_mint), (pubkey(1), pubkey(2)));
        assert_eq!(order.buy_destination, pubkey(4));
        assert_eq!((order.sell_amount, order.buy_amount), (1_000, 2_000));
        assert_eq!(order.amount, 1_000);
        assert_eq!(
            (order.full_sell_amount, order.full_buy_amount),
            (1_000, 2_000)
        );
        assert_eq!(order.side, dex::Side::Sell);
    }
}
