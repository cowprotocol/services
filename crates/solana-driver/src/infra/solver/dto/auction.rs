//! Outbound `/solve` request: the auction the driver posts to a solver engine.
//!
//! The wire format matches `solana-solvers/src/dto/auction.rs`.

use {
    crate::{
        domain::{self, Side, order_uid::OrderUid, solver_fee::SolverFee},
        infra::blockchain::associated_token_address,
    },
    cow_settlement_interface::{pda::buffer::find_buffer_pda, token_program::TokenProgram},
    serde::Serialize,
    serde_with::serde_as,
    solana_sdk::pubkey::Pubkey,
    spl_token_interface::native_mint,
    std::collections::HashSet,
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
    /// The account the swap output lands in: the buy-mint buffer, or the
    /// taker's wSOL ATA for an order buying native SOL. The route must leave
    /// that ATA open: the settlement closes it after the swap to unwrap the
    /// payouts.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_destination: Pubkey,
    /// Sell-mint units left to fill: for a sell order the amount to fill,
    /// for a buy order the most the fill may take.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub sell_amount: u64,
    /// Buy-mint units left to fill: for a buy order the amount to fill, for
    /// a sell order the least the fill must deliver.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub buy_amount: u64,
    /// Sell amount for a sell, buy amount for a buy.
    ///
    /// TODO: remove once external solvers read `sellAmount`/`buyAmount`.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub amount: u64,
    /// The signed sell amount, before prior fills and the solver fee.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub full_sell_amount: u64,
    /// The signed buy amount, before prior fills and the solver fee.
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub full_buy_amount: u64,
    pub side: Side,
    /// Whether a fill may stop short of the order amount.
    pub partially_fillable: bool,
    /// True when the order's buy token account does not exist on chain
    /// yet. The settlement creates it and the solver keypair pays its rent,
    /// so the solution should price that rent in.
    ///
    /// TODO(token-2022): a token-2022 account rents more bytes, so once
    /// those mints are supported this boolean becomes a `setupCostLamports`
    /// number and engines stop having to know the rent math.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub missing_buy_token_account: bool,
}

impl Order {
    pub fn target_amount(&self) -> u64 {
        match self.side {
            Side::Sell => self.sell_amount,
            Side::Buy => self.buy_amount,
        }
    }

    /// Build the wire order from a domain order, the taker and the settlement
    /// program id.
    ///
    /// The swap output must land in the buy-mint buffer PDA so that
    /// `FinalizeSettle` can push it to the user's buy token account. Solvers
    /// swap into token accounts, so a native SOL buy goes out as a wSOL buy
    /// into the taker's wSOL ATA.
    ///
    /// The wire format does not specify how the sell tokens are ultimately
    /// used, so the driver defaults `BeginSettle` to pull sell tokens into the
    /// taker's sell ATA. A future optimization can let solvers report per-order
    /// pull destinations so the driver routes directly to their chosen
    /// accounts.
    fn new(
        order: &domain::Order,
        taker: Pubkey,
        program_id: Pubkey,
        fee: Option<SolverFee>,
        missing_buy_token_account: bool,
    ) -> Self {
        let tighten = |side, limit| fee.map_or(limit, |fee| fee.tighten_limit(side, limit));
        let remaining = order.remaining();
        let (sell_amount, buy_amount) = match order.side {
            Side::Sell => (remaining.sell, tighten(Side::Sell, remaining.buy)),
            Side::Buy => (tighten(Side::Buy, remaining.sell), remaining.buy),
        };
        let (buy_mint, buy_destination) = if order.buys_native_sol() {
            (
                native_mint::ID,
                associated_token_address(&taker, &native_mint::ID, TokenProgram::SplToken),
            )
        } else {
            (
                order.buy_token,
                find_buffer_pda(&program_id, &order.buy_token).0,
            )
        };
        Self {
            uid: order.uid,
            sell_mint: order.sell_token,
            buy_mint,
            buy_destination,
            sell_amount,
            buy_amount,
            amount: match order.side {
                Side::Sell => sell_amount,
                Side::Buy => buy_amount,
            },
            full_sell_amount: order.sell_amount,
            full_buy_amount: order.buy_amount,
            side: order.side,
            partially_fillable: order.partially_fillable,
            missing_buy_token_account,
        }
    }
}

impl Auction {
    /// Build the wire auction from the domain auction.
    ///
    /// The `taker` is the solver that signs the settlement transaction, and its
    /// wSOL ATA receives native SOL buys. `program_id` derives the buy-mint
    /// buffer PDA that every other order swaps into. `fee` tightens every
    /// order's limit leg. `missing_buy_token_accounts` marks the orders whose
    /// payout account the settlement creates.
    pub fn new(
        auction: &domain::Auction,
        taker: Pubkey,
        program_id: Pubkey,
        fee: Option<SolverFee>,
        missing_buy_token_accounts: &HashSet<OrderUid>,
    ) -> Self {
        Self {
            id: auction.id.map(|id| id.get()),
            taker,
            orders: auction
                .orders
                .iter()
                .map(|order| {
                    Order::new(
                        order,
                        taker,
                        program_id,
                        fee,
                        missing_buy_token_accounts.contains(&order.uid),
                    )
                })
                .collect(),
            deadline: auction.deadline,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::Side,
        cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER,
        serde_json::json,
    };

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
                "amount": "1000",
                "fullSellAmount": "1000",
                "fullBuyAmount": "2000",
                "side": "sell",
                "partiallyFillable": false,
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
                amount: 1_000,
                full_sell_amount: 1_000,
                full_buy_amount: 2_000,
                side: Side::Sell,
                partially_fillable: false,
                missing_buy_token_account: false,
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
            executed: 0,
        }
    }

    #[test]
    fn without_a_fee_both_legs_are_the_signed_amounts() {
        let order = Order::new(
            &domain_order(Side::Sell),
            pubkey(3),
            pubkey(0xaa),
            None,
            false,
        );
        assert_eq!((order.sell_amount, order.buy_amount), (1_000, 1_000));
        assert_eq!(
            (order.full_sell_amount, order.full_buy_amount),
            (1_000, 1_000)
        );
    }

    #[test]
    fn the_fee_tightens_the_limit_leg() {
        let fee = Some(SolverFee::try_from(500).unwrap());
        let sell = Order::new(
            &domain_order(Side::Sell),
            pubkey(3),
            pubkey(0xaa),
            fee,
            false,
        );
        assert_eq!((sell.sell_amount, sell.buy_amount), (1_000, 1_053));
        assert_eq!(
            (sell.full_sell_amount, sell.full_buy_amount),
            (1_000, 1_000)
        );

        let buy = Order::new(
            &domain_order(Side::Buy),
            pubkey(3),
            pubkey(0xaa),
            fee,
            false,
        );
        assert_eq!((buy.sell_amount, buy.buy_amount), (952, 1_000));
        assert_eq!((buy.full_sell_amount, buy.full_buy_amount), (1_000, 1_000));
    }

    /// 400 of 1000 sold: 600 is left to sell, the 1000 buy limit scales to
    /// 600 and the fee tightens that to 600 / 0.95 = 631.6. The signed
    /// amounts go out untouched.
    #[test]
    fn a_partially_filled_order_sends_the_remaining_legs() {
        let fee = Some(SolverFee::try_from(500).unwrap());
        let sell = domain::Order {
            partially_fillable: true,
            executed: 400,
            ..domain_order(Side::Sell)
        };
        let order = Order::new(&sell, pubkey(3), pubkey(0xaa), fee, false);
        assert_eq!(
            (order.sell_amount, order.buy_amount, order.amount),
            (600, 632, 600)
        );
        assert_eq!(
            (order.full_sell_amount, order.full_buy_amount),
            (1_000, 1_000)
        );
        assert!(order.partially_fillable);
    }

    #[test]
    fn missing_buy_token_account_is_on_the_wire_only_when_flagged() {
        let (taker, program_id) = (pubkey(3), pubkey(0xaa));
        let order = domain_order(Side::Sell);

        let flagged =
            serde_json::to_value(Order::new(&order, taker, program_id, None, true)).unwrap();
        assert_eq!(flagged["missingBuyTokenAccount"], json!(true));

        let unflagged =
            serde_json::to_value(Order::new(&order, taker, program_id, None, false)).unwrap();
        assert!(unflagged.get("missingBuyTokenAccount").is_none());
    }

    #[test]
    fn a_token_buy_lands_in_the_buffer_pda() {
        let (taker, program_id) = (pubkey(3), pubkey(0xaa));
        let order = Order::new(&domain_order(Side::Sell), taker, program_id, None, false);
        assert_eq!(order.buy_mint, pubkey(2));
        assert_eq!(
            order.buy_destination,
            find_buffer_pda(&program_id, &pubkey(2)).0
        );
    }

    #[test]
    fn a_native_sol_buy_goes_out_as_wsol_into_the_takers_ata() {
        let (taker, program_id) = (pubkey(3), pubkey(0xaa));
        let native_buy = domain::Order {
            buy_token: ENCODED_NATIVE_SOL_TRANSFER,
            ..domain_order(Side::Sell)
        };
        let order = Order::new(&native_buy, taker, program_id, None, false);
        assert_eq!(order.buy_mint, native_mint::ID);
        assert_eq!(
            order.buy_destination,
            associated_token_address(&taker, &native_mint::ID, TokenProgram::SplToken)
        );
    }
}
