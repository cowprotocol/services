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

/// `owner`'s order that `trade` fills exactly, in place of the placeholder
/// `quoted`. The orderbook creates the same order in simulation, see
/// [`QuoteOrder`].
pub(crate) fn order(
    program_id: &Pubkey,
    quoted: &Order,
    trade: &Trade,
    owner: Pubkey,
    sell_program: TokenProgram,
    buy_program: Option<TokenProgram>,
) -> Result<Order, settlement::Error> {
    let quote = QuoteOrder {
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
    let intent = quote
        .intent()
        .ok_or(settlement::Error::ZeroAmount(quoted.uid))?;
    let uid = intent.uid();
    Ok(Order {
        uid: OrderUid(uid.to_bytes()),
        owner,
        sell_token: quoted.sell_token,
        buy_token: quoted.buy_token,
        sell_token_account: quote.sell_token_account(),
        buy_token_account: quote.buy_account(),
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
    use {super::*, cow_settlement_interface::data::intent::OrderIntent, solana_sdk::hash::Hash};

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    /// The settlement derives the intent from the order's fields, so they
    /// must reproduce the uid and PDA of the shared quote order, which the
    /// orderbook creates.
    #[test]
    fn the_order_reproduces_the_shared_quote_orders_uid() {
        let program_id = pubkey(0xaa);
        let quoted = Order {
            uid: OrderUid([0; 32]),
            owner: Pubkey::default(),
            sell_token: pubkey(0x33),
            buy_token: pubkey(0x44),
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
        };
        let trade = Trade {
            order_uid: OrderUid([0; 32]),
            executed_sell: 1_000,
            executed_buy: 2_000,
            solver_fee: 0,
        };
        let order = order(
            &program_id,
            &quoted,
            &trade,
            pubkey(0x11),
            TokenProgram::SplToken,
            Some(TokenProgram::Token2022),
        )
        .unwrap();
        assert_eq!((order.sell_amount, order.buy_amount), (1_000, 2_000));
        assert!(!order.partially_fillable);
        let uid = OrderIntent::try_from(&order).unwrap().uid();
        assert_eq!(Hash::new_from_array(order.uid.0), uid);
        assert_eq!(order.order_pda, find_order_pda(&program_id, &uid).0);
    }
}
