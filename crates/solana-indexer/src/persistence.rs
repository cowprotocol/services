#![expect(dead_code, unused_variables)]
//! PostgreSQL persistence layer for decoded events and slot state.

use {
    crate::types::{
        commitment::{Commitment, UnfinalizedRow},
        errors::PersistenceError,
        events::DecodedEvent,
        recovery::PdaSnapshot,
        slot::Slot,
    },
    std::ops::RangeInclusive,
};

/// PostgreSQL persistence. Used by Decoder, Watchdog, and FinalizationWorker.
///
/// Cheap to clone: wraps a shared pool. The method bodies are stubbed until the
/// Postgres adapter lands.
// TODO: hold `postgres: Arc<Postgres>` once the adapter is added.
#[derive(Clone)]
pub(crate) struct Persistence {}

impl Persistence {
    /// Save decoded events and advance the slot watermark atomically.
    pub(crate) async fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        new_watermark: u64,
    ) -> Result<(), PersistenceError> {
        todo!()
    }

    /// Record a slot checkpoint. Rejects downward writes.
    pub(crate) async fn write_watermark(&self, slot: u64) -> Result<(), PersistenceError> {
        todo!()
    }

    /// Read persisted watermark for resuming after reconnect.
    pub(crate) async fn read_watermark(&self) -> Result<Option<u64>, PersistenceError> {
        todo!()
    }

    /// Record gaps that fell outside the replay window (write-only in v0.1).
    pub(crate) async fn record_lost_slot_range(
        &self,
        range: RangeInclusive<u64>,
    ) -> Result<(), PersistenceError> {
        todo!()
    }

    /// Primary promotion pass: fetch `confirmed` rows whose `slot` is old
    /// enough to be finalized (typically `slot <= tip - 32`) but new enough to
    /// still be within the RPC signature-status retention horizon.
    ///
    /// `limit` is a DB fetch bound (page size), not the RPC batch size. The
    /// finalization worker chunks the returned rows into <=256-signature
    /// `getSignatureStatuses` calls. Returns `Err` on backend failure so the
    /// caller can back off rather than silently stall on a dead store.
    pub(crate) async fn get_confirmed_rows(
        &self,
        max_slot: Slot,
        limit: usize,
    ) -> Result<Vec<UnfinalizedRow>, PersistenceError> {
        todo!()
    }

    /// Safety-net sweep for `confirmed` rows the primary promotion pass missed
    /// (i.e. rows that aged past the signature-status retention horizon,
    /// ~150 slots behind the chain tip).  Returns `Err` on backend failure
    /// (see `get_confirmed_rows`).
    pub(crate) async fn get_aged_rows(
        &self,
        retention_horizon_slot: u64,
    ) -> Result<Vec<UnfinalizedRow>, PersistenceError> {
        todo!()
    }

    /// Flip the `commitment` label on a specific row.
    ///
    /// The row's `table` field tells the implementer which `solana.*` table to
    /// UPDATE.
    pub(crate) async fn update_commitment(
        &self,
        row: &UnfinalizedRow,
        new_commitment: Commitment,
    ) -> Result<(), PersistenceError> {
        todo!()
    }

    /// Persist a single event during recovery/backfills, not the live ingestion
    /// path.
    ///
    /// Unlike `persist_events`, this does not advance the watermark.
    pub(crate) async fn backfill_event(&self, event: DecodedEvent) -> Result<(), PersistenceError> {
        todo!()
    }

    /// Upsert on-chain PDA state for reconciliation.
    pub(crate) async fn upsert_pda_snapshot(
        &self,
        snapshot: PdaSnapshot,
    ) -> Result<(), PersistenceError> {
        todo!()
    }
}
