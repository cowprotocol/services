//! PostgreSQL persistence layer for decoded events and slot state.

use {
    crate::types::{
        commitment::{Commitment, UnfinalizedRow},
        dead_letter::DeadLetterEntry,
        errors::StoreError,
        events::DecodedEvent,
        recovery::PdaSnapshot,
    },
    std::{future::Future, ops::Range},
};

/// PostgreSQL persistence. Used by Decoder, Watchdog, and FinalizationWorker.
///
/// `Send + Sync` so store instances can be shared across async tasks, and each
/// method returns a `Send` future so callers like `Ingester::serve` can be
/// `tokio::spawn`ed. Implementors may still write the bodies as `async fn`; the
/// compiler enforces that the resulting future is `Send`.
pub trait Store: Send + Sync {
    /// Save decoded events and advance the slot watermark atomically.
    fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        new_watermark: u64,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Record a slot checkpoint. Rejects downward writes.
    fn write_watermark(&self, slot: u64) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Read persisted watermark for resuming after reconnect.
    fn read_watermark(&self) -> impl Future<Output = Result<Option<u64>, StoreError>> + Send;

    /// Move stale partials (>32 slots behind) to dead letter table.
    fn write_dead_letter(
        &self,
        entry: DeadLetterEntry,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Record gaps that fell outside the replay window (write-only in v0.1).
    fn record_lost_slot_range(
        &self,
        range: Range<u64>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Primary promotion pass: fetch `confirmed` rows whose `slot` is at or
    /// above the finalization-window threshold (`slot >= newer_than_slot`).
    /// `limit` caps the batch at 256 (RPC batch size). Returns `Err` on
    /// backend failure so the caller can back off rather than
    /// silently stall on a dead store.
    fn get_confirmed_rows(
        &self,
        newer_than_slot: u64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<UnfinalizedRow>, StoreError>> + Send;

    /// Safety-net sweep for `confirmed` rows the primary promotion pass missed
    /// (i.e. rows that aged past the signature-status retention horizon,
    /// ~150 slots behind the chain tip).  Returns `Err` on backend failure
    /// (see `get_confirmed_rows`).
    fn get_aged_rows(
        &self,
        retention_horizon_slot: u64,
    ) -> impl Future<Output = Result<Vec<UnfinalizedRow>, StoreError>> + Send;

    /// Flip the `commitment` label on a specific row.
    ///
    /// The row's `table` field tells the implementer which `solana.*` table to
    /// UPDATE.
    fn update_commitment(
        &self,
        row: &UnfinalizedRow,
        new_commitment: Commitment,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Persist a single event during recovery/backfills, not the live ingestion
    /// path.
    ///
    /// Unlike `persist_events`, this does not advance the watermark.
    fn backfill_event(
        &self,
        event: DecodedEvent,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Upsert on-chain PDA state for reconciliation.
    fn upsert_pda_snapshot(
        &self,
        snapshot: PdaSnapshot,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;
}
