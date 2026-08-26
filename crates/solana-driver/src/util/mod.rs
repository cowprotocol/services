//! Small shared helpers internal to the driver crate.

use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

/// SPL Associated Token Account program ID
/// (`ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`).
const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// SPL Token program ID (`TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`).
pub(crate) const SPL_TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// System program ID (`11111111111111111111111111111111`).
pub(crate) const SYSTEM_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("11111111111111111111111111111111");

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

/// The idempotent instruction creating `owner`'s ATA for `mint` under the SPL
/// Token program, paid for by `payer`. A no-op on chain when the ATA already
/// exists, so concurrent settlements creating the same ATA cannot conflict.
///
/// Shares the token-2022 limitation of [`associated_token_address`]: the SPL
/// Token program id is hard-coded as the mint's token program.
pub fn create_associated_token_account_idempotent(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Instruction {
    spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
        payer,
        owner,
        mint,
        &SPL_TOKEN_PROGRAM_ID,
    )
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

    /// The create instruction targets the same ATA this module derives (the
    /// interface crate and `associated_token_address` must agree on the token
    /// program in the seeds) and encodes the `CreateIdempotent` variant.
    #[test]
    fn create_instruction_targets_the_derived_ata() {
        let payer = Pubkey::from_str_const("11111111111111111111111111111111");
        let mint = Pubkey::from_str_const("So11111111111111111111111111111111111111112");
        let instruction = create_associated_token_account_idempotent(&payer, &payer, &mint);
        assert_eq!(
            instruction.program_id,
            SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID
        );
        assert_eq!(
            instruction.accounts[1].pubkey,
            associated_token_address(&payer, &mint),
        );
        assert_eq!(instruction.data, [1]);
    }
}
