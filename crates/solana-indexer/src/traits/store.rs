#![expect(dead_code)]
//! PostgreSQL persistence layer for decoded events and slot state.

use {
    crate::types::{
        commitment::{Commitment, UnfinalizedRow},
        dead_letter::DeadLetterEntry,
        errors::StoreError,
        events::DecodedEvent,
        recovery::PdaSnapshot,
    },
    std::ops::RangeInclusive,
};

/// PostgreSQL persistence. Used by Decoder, Watchdog, and FinalizationWorker.
pub(crate) trait Store {
    /// Save decoded events and advance the slot watermark atomically.
    async fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        new_watermark: u64,
    ) -> Result<(), StoreError>;

    /// Record a slot checkpoint. Rejects downward writes.
    async fn write_watermark(&self, slot: u64) -> Result<(), StoreError>;

    /// Read persisted watermark for resuming after reconnect.
    async fn read_watermark(&self) -> Result<Option<u64>, StoreError>;

    /// Move stale partials (>32 slots behind) to dead letter table.
    async fn write_dead_letter(&self, entry: DeadLetterEntry) -> Result<(), StoreError>;

    /// Record gaps that fell outside the replay window (write-only in v0.1).
    async fn record_lost_slot_range(&self, range: RangeInclusive<u64>) -> Result<(), StoreError>;

    /// Primary promotion pass: fetch `confirmed` rows whose `slot` is at or
    /// above the finalization-window threshold (`slot >= newer_than_slot`).
    /// `limit` caps the batch at 256 (RPC batch size). Returns `Err` on
    /// backend failure so the caller can back off rather than
    /// silently stall on a dead store.
    async fn get_confirmed_rows(
        &self,
        newer_than_slot: u64,
        limit: usize,
    ) -> Result<Vec<UnfinalizedRow>, StoreError>;

    /// Safety-net sweep for `confirmed` rows the primary promotion pass missed
    /// (i.e. rows that aged past the signature-status retention horizon,
    /// ~150 slots behind the chain tip).  Returns `Err` on backend failure
    /// (see `get_confirmed_rows`).
    async fn get_aged_rows(
        &self,
        retention_horizon_slot: u64,
    ) -> Result<Vec<UnfinalizedRow>, StoreError>;

    /// Flip the `commitment` label on a specific row.
    ///
    /// The row's `table` field tells the implementer which `solana.*` table to
    /// UPDATE.
    async fn update_commitment(
        &self,
        row: &UnfinalizedRow,
        new_commitment: Commitment,
    ) -> Result<(), StoreError>;

    /// Persist a single event during recovery/backfills, not the live ingestion
    /// path.
    ///
    /// Unlike `persist_events`, this does not advance the watermark.
    async fn backfill_event(&self, event: DecodedEvent) -> Result<(), StoreError>;

    /// Upsert on-chain PDA state for reconciliation.
    async fn upsert_pda_snapshot(&self, snapshot: PdaSnapshot) -> Result<(), StoreError>;
}
