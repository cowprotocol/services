//! Outbound `/solve` request: the auction the driver posts to a solver engine.
//!
//! The wire format matches `solana-solvers/src/dto/auction.rs`.

use {
    crate::domain::{self, Side, order_uid::OrderUid, solver_fee::SolverFee},
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
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub sell_amount: u64,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_amount: u64,
    pub side: Side,
}

impl Order {
    pub fn target_amount(&self) -> u64 {
        match self.side {
            Side::Sell => self.sell_amount,
            Side::Buy => self.buy_amount,
        }
    }

    /// Build the wire order from a domain order and the settlement program id.
    ///
    /// The swap output must land in the buy-mint buffer PDA so that
    /// `FinalizeSettle` can push it to the user's buy token account.
    ///
    /// The wire format does not specify how the sell tokens are ultimately
    /// used, so the driver defaults `BeginSettle` to pull sell tokens into the
    /// taker's sell ATA. A future optimization can let solvers report per-order
    /// pull destinations so the driver routes directly to their chosen
    /// accounts.
    fn new(order: &domain::Order, program_id: Pubkey, fee: Option<SolverFee>) -> Self {
        let tighten = |side, limit| fee.map_or(limit, |fee| fee.tighten_limit(side, limit));
        let (sell_amount, buy_amount) = match order.side {
            Side::Sell => (order.sell_amount, tighten(Side::Sell, order.buy_amount)),
            Side::Buy => (tighten(Side::Buy, order.sell_amount), order.buy_amount),
        };
        Self {
            uid: order.uid,
            sell_mint: order.sell_token,
            buy_mint: order.buy_token,
            buy_destination: find_buffer_pda(&program_id, &order.buy_token).0,
            sell_amount,
            buy_amount,
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
    /// user's buy token account. `fee` tightens every order's limit leg.
    pub fn new(
        auction: &domain::Auction,
        taker: Pubkey,
        program_id: Pubkey,
        fee: Option<SolverFee>,
    ) -> Self {
        Self {
            id: auction.id.map(|id| id.get()),
            taker,
            orders: auction
                .orders
                .iter()
                .map(|order| Order::new(order, program_id, fee))
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
                "sellAmount": "1000",
                "buyAmount": "2000",
                "side": "sell",
            }],
            "deadline": "2026-01-01T00:00:00Z",
        });

        let expected = Auction {
            id: Some(1),
            taker,
            orders: vec![Order {
                uid: OrderUid([8; 32]),
                sell_mint: pubkey(1),
                buy_mint,
                buy_destination: find_buffer_pda(&program_id, &buy_mint).0,
                sell_amount: 1_000,
                buy_amount: 2_000,
                side: Side::Sell,
            }],
            deadline: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
        };

        let actual = serde_json::to_value(&expected).unwrap();
        assert_eq!(actual, json);
    }

    fn domain_order(side: Side) -> domain::Order {
        domain::Order {
            uid: OrderUid([8; 32]),
            owner: pubkey(0x22),
            sell_token: pubkey(1),
            buy_token: pubkey(2),
            sell_token_account: pubkey(0x55),
            buy_token_account: pubkey(0x66),
            sell_amount: 1_000,
            buy_amount: 1_000,
            valid_to: u32::MAX,
            side,
            partially_fillable: false,
            order_pda: pubkey(0x67),
            app_data: [0x77; 32],
        }
    }

    #[test]
    fn without_a_fee_both_legs_are_the_signed_amounts() {
        let order = Order::new(&domain_order(Side::Sell), pubkey(0xaa), None);
        assert_eq!((order.sell_amount, order.buy_amount), (1_000, 1_000));
    }

    #[test]
    fn the_fee_tightens_the_limit_leg() {
        let fee = SolverFee::new(500);
        let sell = Order::new(&domain_order(Side::Sell), pubkey(0xaa), fee);
        assert_eq!((sell.sell_amount, sell.buy_amount), (1_000, 1_053));

        let buy = Order::new(&domain_order(Side::Buy), pubkey(0xaa), fee);
        assert_eq!((buy.sell_amount, buy.buy_amount), (952, 1_000));
    }
}
