//! Outbound `/solve` request: the auction the driver posts to a solver engine.
//!
//! The wire format matches `solana-solvers/src/dto/auction.rs`.

use {
    crate::{
        domain::{self, Side, order_uid::OrderUid},
        util::associated_token_address,
    },
    serde::Serialize,
    serde_with::serde_as,
    solana_sdk::pubkey::Pubkey,
};

/// The auction the driver posts to `/solve`.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Auction {
    pub id: i64,
    /// Settlement signer the swap instructions are built for.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub taker: Pubkey,
    pub orders: Vec<Order>,
    /// Absolute deadline by which solutions must be returned.
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// One order to quote.
#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub uid: OrderUid,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub sell_mint: Pubkey,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_mint: Pubkey,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_destination: Pubkey,
    /// Sell amount for a sell, buy amount for a buy
    /// Represented as a quoted decimal string instead of a JSON number.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub amount: u64,
    pub side: Side,
}

impl Order {
    /// Build the wire order from a domain order and the settlement taker.
    ///
    /// The `taker` is the solver that signs the settlement transaction; the
    /// driver derives `buy_destination` as its ATA for the buy mint on the same
    /// premise as the sell token.
    ///
    /// The engine wire carries a single `amount` on the order's side, so the
    /// driver projects the side-matching amount (`sell_amount` for sells,
    /// `buy_amount` for buys) from the full domain order.
    fn from_order_and_taker(order: &domain::Order, taker: Pubkey) -> Self {
        Self {
            uid: order.uid,
            sell_mint: order.sell_token,
            buy_mint: order.buy_token,
            buy_destination: associated_token_address(&taker, &order.buy_token),
            amount: match order.side {
                Side::Sell => order.sell_amount,
                Side::Buy => order.buy_amount,
            },
            side: order.side,
        }
    }
}

impl Auction {
    /// Build the wire auction from the domain auction.
    ///
    /// The `taker` is a concept borrowed from the solana-solvers API: the
    /// solver that signs the settlement transaction. Under the current API the
    /// sell token is the taker's ATA, and the driver derives `buy_destination`
    /// as its buy-side counterpart on the same premise.
    pub fn new(auction: &domain::Auction, taker: Pubkey) -> Self {
        Self {
            id: auction.id.get(),
            taker,
            orders: auction
                .orders
                .iter()
                .map(|order| Order::from_order_and_taker(order, taker))
                .collect(),
            deadline: auction.deadline,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{domain::Side, util},
        serde_json::json,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    /// Pins the outbound `/solve` request shape against the literal the
    /// `solana-solvers` `Auction` deserializes.
    #[test]
    fn wire_format_is_stable() {
        let json = json!({
            "id": 1,
            "taker": pubkey(3).to_string(),
            "orders": [{
                "uid": format!("0x{}", "08".repeat(32)),
                "sellMint": pubkey(1).to_string(),
                "buyMint": pubkey(2).to_string(),
                "buyDestination": util::associated_token_address(&pubkey(3), &pubkey(2)).to_string(),
                "amount": "1000",
                "side": "sell",
            }],
            "deadline": "2026-01-01T00:00:00Z",
        });

        let expected = Auction {
            id: 1,
            taker: pubkey(3),
            orders: vec![Order {
                uid: OrderUid([8; 32]),
                sell_mint: pubkey(1),
                buy_mint: pubkey(2),
                buy_destination: util::associated_token_address(&pubkey(3), &pubkey(2)),
                amount: 1_000,
                side: Side::Sell,
            }],
            deadline: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };

        let actual = serde_json::to_value(&expected).unwrap();
        assert_eq!(actual, json);
    }
}
