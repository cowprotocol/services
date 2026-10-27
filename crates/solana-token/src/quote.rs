//! The order a quote is verified against. The driver settles it and the
//! orderbook creates it in simulation, so both derive it here and agree on
//! its uid.

use {
    cow_settlement_interface::{
        data::intent::{Asset, Flags, OrderIntent, OrderKind, TokenAsset},
        token_program::TokenProgram,
    },
    solana_sdk::pubkey::Pubkey,
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id,
    std::num::NonZeroU64,
};

/// A quote's order: `owner` trades the quoted amounts fill-or-kill through
/// its associated token accounts, or into its wallet for a native SOL buy,
/// never expiring, with empty app data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuoteOrder {
    pub owner: Pubkey,
    pub sell_mint: Pubkey,
    pub sell_program: TokenProgram,
    pub buy_mint: Pubkey,
    /// `None` for a native SOL buy.
    pub buy_program: Option<TokenProgram>,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub kind: OrderKind,
}

impl QuoteOrder {
    /// The owner's associated token account of the sell mint.
    pub fn sell_token_account(&self) -> Pubkey {
        get_associated_token_address_with_program_id(
            &self.owner,
            &self.sell_mint,
            &self.sell_program.address(),
        )
    }

    /// Where the buy amount lands: the owner's associated token account of
    /// the buy mint, or the owner itself for a native SOL buy.
    pub fn buy_account(&self) -> Pubkey {
        match self.buy_program {
            Some(program) => get_associated_token_address_with_program_id(
                &self.owner,
                &self.buy_mint,
                &program.address(),
            ),
            None => self.owner,
        }
    }

    /// The intent, `None` for a zero amount, which the program refuses.
    pub fn intent(&self) -> Option<OrderIntent> {
        Some(OrderIntent {
            owner: self.owner,
            sell: TokenAsset {
                mint: self.sell_mint,
                token_account: self.sell_token_account(),
            },
            buy: Asset::decode(self.buy_mint, self.buy_account()),
            sell_amount: NonZeroU64::new(self.sell_amount)?,
            buy_amount: NonZeroU64::new(self.buy_amount)?,
            valid_to: u32::MAX,
            flags: Flags {
                kind: self.kind,
                partially_fillable: false,
            },
            app_data: [0; 32],
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER};

    fn order(buy_mint: Pubkey, buy_program: Option<TokenProgram>) -> QuoteOrder {
        QuoteOrder {
            owner: Pubkey::new_from_array([0x11; 32]),
            sell_mint: Pubkey::new_from_array([0x33; 32]),
            sell_program: TokenProgram::SplToken,
            buy_mint,
            buy_program,
            sell_amount: 1_000,
            buy_amount: 2_000,
            kind: OrderKind::Sell,
        }
    }

    #[test]
    fn the_intent_trades_through_the_owners_associated_accounts() {
        let order = order(
            Pubkey::new_from_array([0x44; 32]),
            Some(TokenProgram::Token2022),
        );
        let intent = order.intent().unwrap();
        assert_eq!(intent.sell.token_account, order.sell_token_account());
        assert_eq!(
            intent.buy,
            Asset::TokenProgram(TokenAsset {
                mint: order.buy_mint,
                token_account: get_associated_token_address_with_program_id(
                    &order.owner,
                    &order.buy_mint,
                    &spl_token_2022_interface::ID,
                ),
            })
        );
        assert_eq!(intent.valid_to, u32::MAX);
        assert!(!intent.flags.partially_fillable);
    }

    #[test]
    fn a_native_sol_buy_pays_out_to_the_owner() {
        let order = order(ENCODED_NATIVE_SOL_TRANSFER, None);
        assert_eq!(order.buy_account(), order.owner);
        assert_eq!(order.intent().unwrap().buy, Asset::Native(order.owner));
    }

    #[test]
    fn a_zero_amount_has_no_intent() {
        let mut order = order(
            Pubkey::new_from_array([0x44; 32]),
            Some(TokenProgram::SplToken),
        );
        order.buy_amount = 0;
        assert!(order.intent().is_none());
    }
}
