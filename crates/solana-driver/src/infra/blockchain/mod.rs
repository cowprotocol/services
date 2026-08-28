//! The Solana blockchain adapter.
//!
//! Owns the RPC client and the settlement program id. Mirrors the EVM driver's
//! `infra/blockchain/mod.rs` (`struct Ethereum`).

mod accounts;
mod token;

pub use {
    accounts::{AccountsSnapshot, InvalidAddressLookupTableReason, TokenAccountState},
    token::{associated_token_address, create_associated_token_account_idempotent},
};
use {cow_solana_rpc::{Error, SolanaRPC}, solana_sdk::pubkey::Pubkey};

/// The Solana blockchain adapter.
pub struct Solana {
    rpc: SolanaRPC,
    program_id: Pubkey,
}

impl Solana {
    /// Build the adapter from the RPC client and the settlement program id.
    pub fn new(rpc: SolanaRPC, program_id: Pubkey) -> Self {
        Self { rpc, program_id }
    }

    /// The settlement program id this driver settles against.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }

    /// Fetch the accounts at `keys` in a single batched fetch (split into
    /// parallel requests above the server's per-request cap) and return them
    /// as a snapshot ready for typed interpretation.
    pub async fn accounts_snapshot(
        &self,
        keys: impl IntoIterator<Item = Pubkey>,
    ) -> Result<AccountsSnapshot, Error> {
        Ok(AccountsSnapshot::new(
            self.rpc.multiple_accounts(keys).await?,
        ))
    }
}
