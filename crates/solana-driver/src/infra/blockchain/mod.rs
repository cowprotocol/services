//! The Solana blockchain adapter.
//!
//! Owns the RPC client and the settlement program id. Mirrors the EVM driver's
//! `infra/blockchain/mod.rs` (`struct Ethereum`).

use {
    cow_solana_rpc::{Error, SolanaRPC},
    solana_sdk::{
        account::Account,
        hash::Hash,
        pubkey::Pubkey,
        signature::Signature,
        transaction::VersionedTransaction,
    },
    std::collections::HashMap,
};

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

    /// Fetch the latest blockhash and the last block height at which it is
    /// valid.
    pub async fn latest_blockhash(&self) -> Result<(Hash, u64), Error> {
        self.rpc.latest_blockhash().await
    }

    /// Send a signed versioned transaction and return its signature.
    pub async fn send_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature, Error> {
        self.rpc.send_transaction(transaction).await
    }

    /// Fetch accounts by key in a single batched fetch (split into parallel
    /// requests above the server's per-request cap). Accounts that do not
    /// exist are absent from the map.
    pub async fn multiple_accounts(
        &self,
        keys: impl IntoIterator<Item = Pubkey>,
    ) -> Result<HashMap<Pubkey, Account>, Error> {
        self.rpc.multiple_accounts(keys).await
    }
}
