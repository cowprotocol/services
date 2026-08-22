//! Outbound `/solve` request: the auction the driver posts to a solver engine.
//!
//! The wire format matches `solana-solvers/src/dto/auction.rs`.

use {
    crate::domain::{self, Side, order_uid::OrderUid},
    cow_settlement_interface::pda::buffer::find_buffer_pda,
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
    /// Build the wire order from a domain order and the settlement program id.
    ///
    /// The swap output must land in the buy-mint buffer PDA so that
    /// `FinalizeSettle` can push it to the user's buy token account.
    ///
    /// The wire format does not specify how the sell tokens reach the swap
    /// because the current driver defaults `BeginSettle` to pull into the
    /// canonical sell-mint buffer PDA. Solvers must therefore include
    /// transfer instructions in their solutions that move sell tokens from that
    /// buffer PDA into whatever account their swap instructions spend from
    /// (e.g., the taker's sell ATA). A future optimization can let solvers
    /// report per-order pull destinations so the driver routes directly to
    /// their chosen accounts.
    ///
    /// A future optimization can let solvers report per-order pull destinations
    /// so the driver routes directly to their chosen accounts.
    ///
    /// The engine wire carries a single `amount` on the order's side, so the
    /// driver projects the side-matching amount (`sell_amount` for sells,
    /// `buy_amount` for buys) from the full domain order.
    fn from_order_and_program_id(order: &domain::Order, program_id: Pubkey) -> Self {
        Self {
            uid: order.uid,
            sell_mint: order.sell_token,
            buy_mint: order.buy_token,
            buy_destination: find_buffer_pda(&program_id, &order.buy_token).0,
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
    /// The `taker` is the solver that signs the settlement transaction.
    /// `program_id` is used to derive the buy-mint buffer PDA, which is the
    /// swap output destination so `FinalizeSettle` can push it to the
    /// user's buy token account.
    pub fn new(auction: &domain::Auction, taker: Pubkey, program_id: Pubkey) -> Self {
        Self {
            id: auction.id.get(),
            taker,
            orders: auction
                .orders
                .iter()
                .map(|order| Order::from_order_and_program_id(order, program_id))
                .collect(),
            deadline: auction.deadline,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::Side, serde_json::json};

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    /// Pins the outbound `/solve` request shape against the literal the
    /// `solana-solvers` `Auction` deserializes.
    #[test]
    fn wire_format_is_stable() {
        let program_id = pubkey(0xaa);
        let taker = pubkey(3);
        let buy_mint = pubkey(2);
        let json = json!({
            "id": 1,
            "taker": taker.to_string(),
            "orders": [{
                "uid": format!("0x{}", "08".repeat(32)),
                "sellMint": pubkey(1).to_string(),
                "buyMint": buy_mint.to_string(),
                "buyDestination": find_buffer_pda(&program_id, &buy_mint).0.to_string(),
                "amount": "1000",
                "side": "sell",
            }],
            "deadline": "2026-01-01T00:00:00Z",
        });

        let expected = Auction {
            id: 1,
            taker,
            orders: vec![Order {
                uid: OrderUid([8; 32]),
                sell_mint: pubkey(1),
                buy_mint,
                buy_destination: find_buffer_pda(&program_id, &buy_mint).0,
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
