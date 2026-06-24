//! Commitment-tracking types.
//!
//! This module holds the types we use to track how far a transaction has
//! progressed through Solana's commitment pipeline, plus the row shapes the
//! finalization worker reads and writes.
//!
//! The indexer captures transactions at `confirmed` commitment. A later
//! finalization pass polls `getSignatureStatuses` (whose result is modeled by
//! [`SignatureStatus`]) and either promotes the row to `finalized` or marks it
//! `rolled_back`. [`UnfinalizedRow`] is the shape the finalization worker
//! queries for when sweeping aged confirmed rows, and [`AccountInfo`] holds
//! account snapshots used for recovery when accounts aren't obtained normally
//! through the ingestion stream.

use {crate::types::Signature, bytes::Bytes, solana_sdk::pubkey::Pubkey};

/// Commitment level persisted by the indexer.
///
/// Solana consensus defines `processed`, `confirmed`, and `finalized`
/// commitment levels, but we only store the two durable states plus a terminal
/// failure state for abandoned slots. `processed` is omitted because it
/// reflects the node's latest view and is still rollback-prone.
#[derive(Debug, Clone, Copy)]
pub enum Commitment {
    /// Voted on by a supermajority but can still be rolled back. Watched by the
    /// finalization worker.
    Confirmed,
    /// Rooted by the cluster and considered permanently settled.
    Finalized,
    /// Never landed, or its slot was abandoned by the cluster.
    RolledBack,
}

impl Commitment {
    /// String label used in `solana.*` `commitment` columns.
    pub fn as_label(self) -> &'static str {
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
    pub data: Bytes,
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
