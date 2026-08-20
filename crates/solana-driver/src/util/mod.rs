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
/// TODO(token-2022): this derivation hard-codes the SPL Token program ID as the
/// mint's token program in the PDA seed. Token-2022 mints live under a
/// different program, whose ATA addresses derive with a different seed set, so
/// this returns the wrong address for them.
///
/// To support token-2022 we'd need to look up the mint's token program (e.g.
/// via `get_account_info` on the mint) and use that program ID in the seeds.
pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), SPL_TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID,
    )
    .0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cross-checked against the known WSOL ATA for the system program owner
    /// on Solana mainnet.
    #[test]
    fn derives_associated_token_address() {
        let owner = Pubkey::from_str_const("11111111111111111111111111111111");
        let mint = Pubkey::from_str_const("So11111111111111111111111111111111111111112");
        assert_eq!(
            associated_token_address(&owner, &mint),
            Pubkey::from_str_const("aqxoAhCwpy3oB1BpNw9hL1HdLYLgPpbPjzxDrrQj3Fs"),
        );
    }
}
