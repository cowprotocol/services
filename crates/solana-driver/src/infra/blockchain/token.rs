//! SPL token account helpers.
//!
//! The blockchain adapter keeps these helpers because they encode
//! chain-specific program IDs and ATA derivation rules.

use {
    solana_sdk::{instruction::Instruction, pubkey::Pubkey},
    spl_token_interface::ID as SPL_TOKEN_PROGRAM_ID,
};

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The system-program owner + the WSOL mint derive to a golden (known-good)
    /// ATA. For now, this test guards against token-2022 addresses, as a wrong
    /// token program id or seed results in a faliure.
    #[test]
    fn derives_associated_token_address() {
        const GOLDEN_WSOL_ATA: Pubkey =
            Pubkey::from_str_const("aqxoAhCwpy3oB1BpNw9hL1HdLYLgPpbPjzxDrrQj3Fs");

        let owner = solana_system_interface::program::ID;
        let mint = spl_token_interface::native_mint::ID;
        assert_eq!(associated_token_address(&owner, &mint), GOLDEN_WSOL_ATA);
    }
}
