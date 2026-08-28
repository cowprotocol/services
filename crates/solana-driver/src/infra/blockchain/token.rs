//! SPL token account helpers.
//!
//! The blockchain adapter keeps these helpers because they encode
//! chain-specific program IDs and ATA derivation rules.

use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

#[allow(dead_code)]
const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

pub(crate) const SPL_TOKEN_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub(crate) const SYSTEM_PROGRAM_ID: Pubkey =
    Pubkey::from_str_const("11111111111111111111111111111111");

/// Derive the ATA address for `owner` and `mint` under the SPL Token program.
///
/// TODO(token-2022): this function hard-codes the SPL Token program ID as the
/// mint's token program in the PDA seed. Token-2022 mints live under a
/// different program. The ATA addresses for those mints use a different seed
/// set. So this function returns the wrong address for them.
///
/// To support token-2022, look up the mint's token program (for example through
/// `get_account_info` on the mint). Then use that program ID in the seeds.
pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        owner,
        mint,
        &SPL_TOKEN_PROGRAM_ID,
    )
}

/// Create the idempotent instruction that makes `owner`'s ATA for `mint` under
/// the SPL Token program. The instruction is a no-op on chain when the ATA
/// already exists. This means concurrent settlements that create the same ATA
/// cannot conflict.
///
/// This function has the same token-2022 limitation as
/// [`associated_token_address`]
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
