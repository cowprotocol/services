#![expect(dead_code)]
//! The finalization worker updates the commitment level of the transactions
//! tracked by the indexer, promoting rows written at `confirmed` to
//! `finalized`.
//!
//! It does so through two flows. Two are needed because the relevant RPC
//! methods trade off differently: `getSignatureStatuses` is batchable but the
//! node only retains statuses for recent slots, while `getTransaction` reaches
//! arbitrarily old transactions on archival nodes but costs one call per
//! signature. The batched pass handles the common case cheaply; the per-row
//! sweep catches rows that age out of it.
//!
//! - **Promotion pass**: batch-polls `getSignatureStatuses` (at most
//!   [`PROMOTION_BATCH_LIMIT`] signatures per call) over rows still at
//!   `confirmed` that are at least [`FINALIZATION_WINDOW_SLOTS`] behind the
//!   chain tip, and promotes rows whose `confirmationStatus` is `"finalized"`.
//!
//! - **Aged-row sweep**: fallback for rows past the signature-status retention
//!   horizon ([`SIGNATURE_STATUS_RETENTION_SLOTS`]), which the promotion pass
//!   can no longer check. Each row costs one `getTransaction` call; a non-null
//!   response promotes to `finalized`, a null response marks `rolled_back`.

// TODO:  This file only declares the component skeleton. The `run` body is
// `unimplemented!`; both flows arrive in a later change.

use {
    crate::{persistence::Persistence, traits::solana_client::SolanaClient},
    std::sync::Arc,
};

/// Slots a transaction usually needs to finalize (~12.8 s at 400 ms/slot).
/// A heuristic floor, not a guarantee: the promotion pass skips rows fresher
/// than this because they cannot have finalized yet, and degraded consensus
/// can push real finalization later (the aged-row sweep catches those).
pub const FINALIZATION_WINDOW_SLOTS: u64 = 32;

/// Upper limit for the `getSignatureStatuses` batch RPC call.
pub const PROMOTION_BATCH_LIMIT: usize = 256;

/// Approximate slot horizon past which `getSignatureStatuses` no longer returns
/// a result.
pub const SIGNATURE_STATUS_RETENTION_SLOTS: u64 = 150;

/// Transaction finalization worker. See the module docs for the two flows it
/// runs.
pub(crate) struct FinalizationWorker {
    /// Persistence layer.
    pub persistence: Persistence,

    /// RPC implementor.
    pub rpc: Arc<dyn SolanaClient>,
}

impl FinalizationWorker {
    /// Construct a new finalization worker.
    pub fn new(persistence: Persistence, rpc: Arc<dyn SolanaClient>) -> Self {
        Self { persistence, rpc }
    }

    /// Outer loop. Runs the promotion pass and the aged-row sweep on a timer.
    ///
    /// Placeholder for now; implemented in a later change.
    pub async fn run(&mut self) {
        unimplemented!("implemented in PR 11–12")
    }
}
