//! The order a quote's settlement is built against.

use {
    super::{Order, Side, Trade, order_uid::OrderUid, settlement},
    cow_settlement_interface::{
        data::intent::OrderKind,
        pda::order::find_order_pda,
        token_program::TokenProgram,
    },
    solana_sdk::pubkey::Pubkey,
    solana_token::quote::QuoteOrder,
};

/// The order `trade` fills exactly at its executed amounts, in place of the
/// placeholder `quoted` was solved as, owned by `owner`. The orderbook
/// creates the same order in simulation, see [`QuoteOrder`]. `buy_program`
/// is `None` for a native SOL buy.
pub(crate) fn order(
    program_id: &Pubkey,
    quoted: &Order,
    trade: &Trade,
    owner: Pubkey,
    sell_program: TokenProgram,
    buy_program: Option<TokenProgram>,
) -> Result<Order, settlement::Error> {
    let fabricated = QuoteOrder {
        owner,
        sell_mint: quoted.sell_token,
        sell_program,
        buy_mint: quoted.buy_token,
        buy_program,
        sell_amount: trade.executed_sell,
        buy_amount: trade.executed_buy,
        kind: match quoted.side {
            Side::Sell => OrderKind::Sell,
            Side::Buy => OrderKind::Buy,
        },
    };
    let intent = fabricated
        .intent()
        .ok_or(settlement::Error::ZeroAmount(quoted.uid))?;
    let uid = intent.uid();
    Ok(Order {
        uid: OrderUid(uid.to_bytes()),
        owner,
        sell_token: quoted.sell_token,
        buy_token: quoted.buy_token,
        sell_token_account: fabricated.sell_token_account(),
        buy_token_account: fabricated.buy_account(),
        sell_amount: trade.executed_sell,
        buy_amount: trade.executed_buy,
        valid_to: u32::MAX,
        side: quoted.side,
        partially_fillable: false,
        order_pda: find_order_pda(program_id, &uid).0,
        app_data: [0; 32],
        executed: 0,
        sell_balance: None,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        cow_settlement_interface::data::intent::{ENCODED_NATIVE_SOL_TRANSFER, OrderIntent},
        solana_sdk::hash::Hash,
        spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn quoted(buy_token: Pubkey) -> Order {
        Order {
            uid: OrderUid([0; 32]),
            owner: Pubkey::default(),
            sell_token: pubkey(0x33),
            buy_token,
            sell_token_account: Pubkey::default(),
            buy_token_account: Pubkey::default(),
            sell_amount: 1_000,
            buy_amount: 0,
            valid_to: u32::MAX,
            side: Side::Sell,
            partially_fillable: false,
            order_pda: Pubkey::default(),
            app_data: [0; 32],
            executed: 0,
            sell_balance: None,
        }
    }

    fn trade() -> Trade {
        Trade {
            order_uid: OrderUid([0; 32]),
            executed_sell: 1_000,
            executed_buy: 2_000,
            solver_fee: 0,
        }
    }

    /// The order fills exactly the executed amounts through the owner's
    /// associated token accounts under each mint's program, and the intent
    /// the settlement derives from it is the shared one: same uid, same PDA.
    #[test]
    fn the_order_fills_the_trade_exactly_through_the_owners_atas() {
        let (program_id, owner) = (pubkey(0xaa), pubkey(0x11));
        let order = order(
            &program_id,
            &quoted(pubkey(0x44)),
            &trade(),
            owner,
            TokenProgram::SplToken,
            Some(TokenProgram::Token2022),
        )
        .unwrap();
        assert_eq!((order.sell_amount, order.buy_amount), (1_000, 2_000));
        assert!(!order.partially_fillable);
        assert_eq!(order.owner, owner);
        assert_eq!(
            order.sell_token_account,
            get_associated_token_address_with_program_id(
                &owner,
                &pubkey(0x33),
                &spl_token_interface::ID
            )
        );
        assert_eq!(
            order.buy_token_account,
            get_associated_token_address_with_program_id(
                &owner,
                &pubkey(0x44),
                &spl_token_2022_interface::ID
            )
        );
        let uid = OrderIntent::try_from(&order).unwrap().uid();
        assert_eq!(Hash::new_from_array(order.uid.0), uid);
        assert_eq!(order.order_pda, find_order_pda(&program_id, &uid).0);
    }

    #[test]
    fn a_native_sol_buy_pays_out_to_the_wallet() {
        let order = order(
            &pubkey(0xaa),
            &quoted(ENCODED_NATIVE_SOL_TRANSFER),
            &trade(),
            pubkey(0x11),
            TokenProgram::SplToken,
            None,
        )
        .unwrap();
        assert_eq!(order.buy_token_account, order.owner);
    }

    #[test]
    fn a_zero_leg_has_no_order() {
        let trade = Trade {
            executed_buy: 0,
            ..trade()
        };
        assert!(matches!(
            order(
                &pubkey(0xaa),
                &quoted(pubkey(0x44)),
                &trade,
                pubkey(0x11),
                TokenProgram::SplToken,
                Some(TokenProgram::SplToken),
            ),
            Err(settlement::Error::ZeroAmount(_))
        ));
    }
}
