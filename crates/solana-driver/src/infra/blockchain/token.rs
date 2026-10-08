//! Token account helpers for the SPL Token and Token-2022 programs.
//!
//! The blockchain adapter keeps these helpers because they encode
//! chain-specific program IDs and ATA derivation rules.

use {
    cow_settlement_interface::token_program::TokenProgram,
    solana_sdk::{instruction::Instruction, pubkey::Pubkey},
    spl_token_interface::ID as SPL_TOKEN_PROGRAM_ID,
};

/// Derive the ATA address for `owner` and `mint` under the mint's token
/// `program`. The program is one of the seeds, so the two token programs
/// derive different addresses for the same owner and mint.
pub fn associated_token_address(owner: &Pubkey, mint: &Pubkey, program: TokenProgram) -> Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        owner,
        mint,
        &program.address(),
    )
}

/// Create the idempotent instruction that makes `owner`'s ATA for `mint` under
/// the mint's token `program`. The instruction is a no-op on chain when the
/// ATA already exists. This means concurrent settlements that create the same
/// ATA cannot conflict.
pub fn create_associated_token_account_idempotent(
    payer: &Pubkey,
    owner: &Pubkey,
    mint: &Pubkey,
    program: TokenProgram,
) -> Instruction {
    spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent(
        payer,
        owner,
        mint,
        &program.address(),
    )
}

/// Create the instruction that closes `owner`'s token `account` under the SPL
/// Token program and sends its lamports to `destination`. Closing a wSOL
/// account unwraps its whole balance.
pub fn close_token_account(account: &Pubkey, destination: &Pubkey, owner: &Pubkey) -> Instruction {
    spl_token_interface::instruction::close_account(
        &SPL_TOKEN_PROGRAM_ID,
        account,
        destination,
        owner,
        &[],
    )
    .expect("the SPL Token program id passes the program check")
}

/// Create the instruction that fails unless `owner`'s token `account` under the
/// SPL Token program holds at least `amount`. It is a self-transfer, which
/// checks the balance and moves nothing.
pub fn require_token_balance(account: &Pubkey, owner: &Pubkey, amount: u64) -> Instruction {
    spl_token_interface::instruction::transfer(
        &SPL_TOKEN_PROGRAM_ID,
        account,
        account,
        owner,
        &[],
        amount,
    )
    .expect("the SPL Token program id passes the program check")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The system-program owner + the WSOL mint derive to a golden (known-good)
    /// SPL Token ATA. A wrong token program id or seed fails the comparison.
    #[test]
    fn derives_associated_token_address() {
        const GOLDEN_WSOL_ATA: Pubkey =
            Pubkey::from_str_const("aqxoAhCwpy3oB1BpNw9hL1HdLYLgPpbPjzxDrrQj3Fs");

        let owner = solana_system_interface::program::ID;
        let mint = spl_token_interface::native_mint::ID;
        assert_eq!(
            associated_token_address(&owner, &mint, TokenProgram::SplToken),
            GOLDEN_WSOL_ATA
        );
    }

    /// A PYUSD holder's ATA on mainnet, a golden Token-2022 address. The same
    /// owner and mint derive elsewhere under the SPL Token program.
    #[test]
    fn derives_a_token_2022_associated_token_address() {
        const GOLDEN_PYUSD_ATA: Pubkey =
            Pubkey::from_str_const("4J3DEadaxaVp194Z7HqyxG7NenFtwQHP3iPrunErnRJc");

        let owner = Pubkey::from_str_const("DcQNaByvstRZCERvb9UJHgHiQe8YX2FMdPptM86psQyz");
        let mint = Pubkey::from_str_const("2b1kV6DkPAnxd5ixfnxCpjxmKwqjjaYmCZfHsFu24GXo");
        assert_eq!(
            associated_token_address(&owner, &mint, TokenProgram::Token2022),
            GOLDEN_PYUSD_ATA
        );
        assert_ne!(
            associated_token_address(&owner, &mint, TokenProgram::SplToken),
            GOLDEN_PYUSD_ATA
        );
    }
}
