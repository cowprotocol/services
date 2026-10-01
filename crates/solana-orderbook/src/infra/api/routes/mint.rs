//! The mint checks quoting and placement share.

use {
    crate::infra::api::error,
    axum::http::StatusCode,
    cow_settlement_interface::{
        data::intent::ENCODED_NATIVE_SOL_TRANSFER,
        token_program::TokenProgram,
    },
    solana_sdk::pubkey::Pubkey,
    solana_token::{MintVerdict, UnsettleableMint},
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
/// cannot move, by its verdict in `verdicts`. A mint without a verdict counts
/// as missing.
pub(super) fn ensure_settleable(
    verdicts: &HashMap<Pubkey, MintVerdict>,
    mints: impl IntoIterator<Item = Pubkey>,
) -> Result<(), error::Reply> {
    mints.into_iter().try_for_each(|mint| {
        let verdict = verdicts.get(&mint).copied();
        match verdict.unwrap_or(Err(UnsettleableMint::NotAMint)) {
            Ok(_) => Ok(()),
            Err(reason) => Err(error::reply(
                StatusCode::BAD_REQUEST,
                "UnsupportedToken",
                format!("Token {mint} is unsupported: {reason}"),
            )),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// `UnsupportedToken` shape. A mint without a verdict is rejected as
    /// missing.
    #[test]
    fn the_first_unsettleable_mint_is_rejected() {
        let good = Pubkey::new_unique();
        let bad = Pubkey::new_unique();
        let verdicts = HashMap::from([
            (good, Ok(TokenProgram::SplToken)),
            (bad, Err(UnsettleableMint::TransferFee)),
        ]);
        assert!(ensure_settleable(&verdicts, [good]).is_ok());
        let (status, body) = ensure_settleable(&verdicts, [good, bad]).unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body.error_type, "UnsupportedToken");
        assert_eq!(
            body.description,
            format!("Token {bad} is unsupported: Token-2022 transfer fee extension")
        );
        let unknown = Pubkey::new_unique();
        let (_, body) = ensure_settleable(&verdicts, [good, unknown]).unwrap_err();
        assert_eq!(
            body.description,
            format!(
                "Token {unknown} is unsupported: not a mint of the SPL Token or Token-2022 program"
            )
        );
    }
}
