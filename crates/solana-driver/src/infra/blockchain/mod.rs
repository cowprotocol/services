//! The Solana blockchain adapter.
//!
//! Owns the RPC client and the settlement program id. Mirrors the EVM driver's
//! `infra/blockchain/mod.rs` (`struct Ethereum`).

use solana_sdk::pubkey::Pubkey;

/// The Solana blockchain adapter.
pub struct Solana {
    #[expect(dead_code, reason = "used by the settlement path in follow-up PRs")]
    rpc: cow_solana_rpc::SolanaRPC,
    program_id: Pubkey,
}

impl Solana {
    /// Build the adapter from the RPC client and the settlement program id.
    pub fn new(rpc: cow_solana_rpc::SolanaRPC, program_id: Pubkey) -> Self {
        Self { rpc, program_id }
    }

    /// The settlement program id this driver settles against.
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }
}
