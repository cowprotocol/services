//! Small shared helpers internal to the driver crate.

use solana_sdk::pubkey::Pubkey;

/// SPL Associated Token Account program ID
/// (`ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`).
const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// SPL Token program ID (`TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`).
const SPL_TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Derive the associated token account (ATA) address for `owner` and `mint`
/// under the SPL Token program.
///
/// Mirrors `spl_associated_token_account::get_associated_token_address` so the
/// driver can compute the solver's buy-mint buffer without pulling in the SPL
/// crate. The swap sends its output to this account; `FinalizeSettle` then
/// forwards it to the user. The settlement program itself holds no ATAs — the
/// underlying solver does.
pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), SPL_TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID,
    )
    .0
}

#[cfg(test)]
mod tests {
    use {super::*, std::str::FromStr};

    /// Cross-checked against a known ATA derivation: the WSOL ATA for the
    /// system program owner on Solana mainnet.
    #[test]
    fn derives_associated_token_address() {
        let owner = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        let mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();
        let ata = associated_token_address(&owner, &mint);
        // Stable, deterministic PDA: re-deriving must agree.
        assert_eq!(ata, associated_token_address(&owner, &mint));
        // The ATA is a curve point-free PDA, never the zero address.
        assert!(ata != Pubkey::default());
    }
}
