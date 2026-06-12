//! Commitment-tracking types: confirmation state, signature status, and the row
//! shapes consumed by the finalization worker.

use {crate::types::Signature, solana_sdk::pubkey::Pubkey};

/// On-chain commitment of a transaction or row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commitment {
    /// The row is at `confirmed` commitment; the finalization worker still has
    /// work to do.
    Confirmed,
    /// The row is at `finalized` commitment.
    Finalized,
    /// The row's transaction never landed (or was rolled back).
    RolledBack,
}

impl Commitment {
    /// String label used in `solana.*` `commitment` columns.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::Finalized => "finalized",
            Self::RolledBack => "rolled_back",
        }
    }
}

/// Result of an RPC `getSignatureStatuses` poll.
#[derive(Debug, Clone, Copy)]
pub struct SignatureStatus {
    /// Slot the transaction landed at, if known.
    pub slot: u64,
    /// Confirmation status reported by the RPC.
    pub confirmation_status: Commitment,
}

/// Snapshot of an account at a given slot (from `getAccountInfo`).
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// Slot the snapshot was read at.
    pub slot: u64,
    /// Account data (serialized).
    pub data: Vec<u8>,
    /// Account owner program.
    pub owner: Pubkey,
}

/// A `solana.*` row that has not yet reached `finalized` commitment — the kind
/// picked up by the aged-row sweep, where `commitment = 'confirmed'` and the
/// row's slot is at least one finalization window behind `LATEST_CHAIN_SLOT`.
#[derive(Debug, Clone)]
pub struct UnfinalizedRow {
    /// Table the row lives in.
    pub table: &'static str,
    /// Transaction signature.
    pub signature: Signature,
    /// Slot the row was inserted at.
    pub slot: u64,
}
