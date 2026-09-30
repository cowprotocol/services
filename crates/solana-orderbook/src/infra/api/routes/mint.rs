//! The mint checks quoting and placement share.

use {
    crate::infra::api::error,
    axum::http::StatusCode,
    cow_settlement_interface::{
        data::intent::ENCODED_NATIVE_SOL_TRANSFER,
        token_program::TokenProgram,
    },
    solana_sdk::{account::Account, pubkey::Pubkey},
    solana_token::unsettleable_mint,
    std::collections::HashMap,
};

/// Whether `program` is one of the two token programs, classic SPL or
/// Token-2022.
pub(super) fn is_token_program(program: &Pubkey) -> bool {
    TokenProgram::try_from(program).is_ok()
}

/// The mints an order moves: the sell mint, and the buy mint unless the order
/// buys native SOL.
pub(super) fn token_mints(sell: Pubkey, buy: Pubkey) -> impl Iterator<Item = Pubkey> {
    std::iter::once(sell).chain((buy != ENCODED_NATIVE_SOL_TRANSFER).then_some(buy))
}

/// Answer `UnsupportedToken` for the first of `mints` the settlement program
/// cannot move, reading each mint from `accounts`.
pub(super) fn ensure_settleable(
    accounts: &HashMap<Pubkey, Account>,
    mints: impl IntoIterator<Item = Pubkey>,
) -> Result<(), error::Reply> {
    mints
        .into_iter()
        .try_for_each(|mint| match unsettleable_mint(accounts.get(&mint)) {
            Some(reason) => Err(error::reply(
                StatusCode::BAD_REQUEST,
                "UnsupportedToken",
                format!("Token {mint} is unsupported: {reason}"),
            )),
            None => Ok(()),
        })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_testlib::{classic_mint, token_2022_mint},
        spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut,
            ExtensionType,
            transfer_fee::TransferFeeConfig,
        },
    };

    /// A native SOL buy has no buy mint to read.
    #[test]
    fn native_sol_buys_have_no_buy_mint() {
        let sell = Pubkey::new_unique();
        let buy = Pubkey::new_unique();
        assert_eq!(token_mints(sell, buy).collect::<Vec<_>>(), [sell, buy]);
        assert_eq!(
            token_mints(sell, ENCODED_NATIVE_SOL_TRANSFER).collect::<Vec<_>>(),
            [sell]
        );
    }

    /// The rejection names the mint and the reason, in the EVM
    /// `UnsupportedToken` shape.
    #[test]
    fn the_first_unsettleable_mint_is_rejected() {
        let good = Pubkey::new_unique();
        let bad = Pubkey::new_unique();
        let accounts = HashMap::from([
            (good, classic_mint(6)),
            (
                bad,
                token_2022_mint(&[ExtensionType::TransferFeeConfig], |mint| {
                    mint.init_extension::<TransferFeeConfig>(true).unwrap();
                }),
            ),
        ]);
        assert!(ensure_settleable(&accounts, [good]).is_ok());
        let (status, body) = ensure_settleable(&accounts, [good, bad]).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.error_type, "UnsupportedToken");
        assert_eq!(
            body.description,
            format!("Token {bad} is unsupported: Token-2022 transfer fee extension")
        );
    }
}
