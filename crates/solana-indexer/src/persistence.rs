#![allow(dead_code, unused_variables)]
//! PostgreSQL persistence layer for decoded events and slot state.

use {
    crate::types::{
        commitment::{Commitment, UnfinalizedRow},
        errors::PersistenceError,
        events::{DecodedEvent, SettlementEvent},
        recovery::PdaSnapshot,
        slot::Slot,
    },
    sqlx::{PgConnection, PgPool, types::BigDecimal},
    std::ops::RangeInclusive,
};

#[cfg(test)]
mod tests;

/// Single-row id for the slot watermark in `solana.indexer_state`.
const WATERMARK_ROW_ID: i32 = 0;

/// PostgreSQL persistence. Used by Decoder, Watchdog, and FinalizationWorker.
///
/// Cheap to clone: `PgPool` is an `Arc` internally.
#[derive(Clone)]
pub(crate) struct Persistence {
    pool: PgPool,
}

impl Persistence {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Test stub: a lazily-connected pool that never touches a DB unless a
    /// query actually runs. Lets components that hold a `Persistence` be built
    /// in unit tests that do not exercise the persistence path.
    #[cfg(test)]
    pub(crate) fn stub() -> Self {
        Self {
            pool: PgPool::connect_lazy("postgresql://localhost").expect("valid lazy url"),
        }
    }

    /// Save decoded events and advance the slot watermark in one transaction.
    pub(crate) async fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        new_watermark: u64,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        for event in events {
            match event {
                DecodedEvent::Settlement(event) => write_settlement_event(&mut tx, event).await?,
                // The SolFlow program is not live yet and its decode is stubbed,
                // so no SolFlow events reach here. Wired once the program lands.
                DecodedEvent::SolFlow(_) => {}
            }
        }
        advance_watermark(&mut tx, new_watermark).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(())
    }

    /// Record a slot checkpoint. Downward writes are ignored (monotonic).
    pub(crate) async fn write_watermark(&self, slot: u64) -> Result<(), PersistenceError> {
        let mut conn = self.pool.acquire().await.map_err(map_err)?;
        advance_watermark(&mut conn, slot).await
    }

    /// Read the persisted watermark for resuming after reconnect.
    pub(crate) async fn read_watermark(&self) -> Result<Option<u64>, PersistenceError> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT last_indexed_slot FROM solana.indexer_state WHERE id = $1")
                .bind(WATERMARK_ROW_ID)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_err)?;
        Ok(row.map(|(slot,)| slot as u64))
    }

    /// Record gaps that fell outside the replay window (write-only in v0.1).
    pub(crate) async fn record_lost_slot_range(
        &self,
        range: RangeInclusive<u64>,
    ) -> Result<(), PersistenceError> {
        todo!("recovery flows are out of scope for v0.1 (database spec §13)")
    }

    /// Primary promotion pass source rows. Implemented with the
    /// FinalizationWorker.
    pub(crate) async fn get_confirmed_rows(
        &self,
        max_slot: Slot,
        limit: usize,
    ) -> Result<Vec<UnfinalizedRow>, PersistenceError> {
        todo!("FinalizationWorker PR")
    }

    /// Aged-row safety-net sweep source. Implemented with the
    /// FinalizationWorker.
    pub(crate) async fn get_aged_rows(
        &self,
        retention_horizon_slot: u64,
    ) -> Result<Vec<UnfinalizedRow>, PersistenceError> {
        todo!("FinalizationWorker PR")
    }

    /// Flip the `commitment` label on a row. Implemented with the
    /// FinalizationWorker.
    pub(crate) async fn update_commitment(
        &self,
        row: &UnfinalizedRow,
        new_commitment: Commitment,
    ) -> Result<(), PersistenceError> {
        todo!("FinalizationWorker PR")
    }

    /// Persist a single event during recovery/backfills. Out of scope for v0.1.
    pub(crate) async fn backfill_event(&self, event: DecodedEvent) -> Result<(), PersistenceError> {
        todo!("recovery flows are out of scope for v0.1 (database spec §13)")
    }

    /// Upsert on-chain PDA state for reconciliation. Out of scope for v0.1.
    pub(crate) async fn upsert_pda_snapshot(
        &self,
        snapshot: PdaSnapshot,
    ) -> Result<(), PersistenceError> {
        todo!("recovery flows are out of scope for v0.1 (database spec §13)")
    }
}

/// Advance the monotonic slot watermark. A slot at or below the stored value is
/// a no-op (the `WHERE` guard on the upsert rejects the downward write).
async fn advance_watermark(conn: &mut PgConnection, slot: u64) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO solana.indexer_state (id, last_indexed_slot) VALUES ($1, $2) ON CONFLICT \
         (id) DO UPDATE SET last_indexed_slot = EXCLUDED.last_indexed_slot WHERE \
         EXCLUDED.last_indexed_slot > solana.indexer_state.last_indexed_slot",
    )
    .bind(WATERMARK_ROW_ID)
    .bind(slot as i64)
    .execute(&mut *conn)
    .await
    .map_err(map_err)?;
    Ok(())
}

/// Persist one settlement-program event.
///
/// Placeholders mirror PR 7.2: `instruction_index` and `fee_amount` are not
/// carried by the current event taxonomy (nor is the buy-side amount emitted by
/// the program yet), so they are written as constants until the decoder
/// resolves them from the proposed-solution data.
async fn write_settlement_event(
    conn: &mut PgConnection,
    event: SettlementEvent,
) -> Result<(), PersistenceError> {
    match event {
        // Off-chain path: `solana.orders` is the orderbook's row; the indexer
        // writes only the on-chain PDA mirror. Pure on-chain creation (which
        // also writes `solana.orders` from the decoded intent) needs the full
        // intent the current event does not carry. Deferred.
        SettlementEvent::OrderCreated {
            order_uid,
            created_by,
            ..
        } => {
            sqlx::query(
                "INSERT INTO solana.order_pda (order_uid, created_by) VALUES ($1, $2) ON CONFLICT \
                 (order_uid) DO NOTHING",
            )
            .bind(order_uid.0.to_vec())
            .bind(created_by.to_bytes().to_vec())
            .execute(&mut *conn)
            .await
            .map_err(map_err)?;
        }
        SettlementEvent::SettlementFinalized {
            auction_id,
            solver,
            tx_signature,
            slot,
            trades,
        } => {
            let tx_sig = tx_signature.as_ref().to_vec();
            sqlx::query(
                "INSERT INTO solana.settlements (slot, tx_signature, solver, auction_id, \
                 solution_uid, commitment) VALUES ($1, $2, $3, $4, NULL, 'confirmed') ON CONFLICT \
                 (tx_signature) DO NOTHING",
            )
            .bind(slot.0 as i64)
            .bind(&tx_sig)
            .bind(solver.to_bytes().to_vec())
            .bind(auction_id as i64)
            .execute(&mut *conn)
            .await
            .map_err(map_err)?;

            for (index, trade) in trades.iter().enumerate() {
                let order_uid = trade.order_uid.0.to_vec();
                let sell = BigDecimal::from(trade.amount_withdrawn_delta);
                let buy = BigDecimal::from(trade.amount_received_delta);
                sqlx::query(
                    "INSERT INTO solana.trades \
                     (settlement_tx_signature, instruction_index, order_uid, sell_amount, \
                      buy_amount, fee_amount, commitment) \
                     VALUES ($1, $2, $3, $4, $5, 0, 'confirmed') \
                     ON CONFLICT DO NOTHING",
                )
                .bind(&tx_sig)
                .bind(index as i32) // placeholder: real instruction_index not carried yet
                .bind(&order_uid)
                .bind(&sell)
                .bind(&buy)
                .execute(&mut *conn)
                .await
                .map_err(map_err)?;

                sqlx::query(
                    "UPDATE solana.order_pda SET amount_withdrawn = amount_withdrawn + $1, \
                     amount_received = amount_received + $2 WHERE order_uid = $3",
                )
                .bind(&sell)
                .bind(&buy)
                .bind(&order_uid)
                .execute(&mut *conn)
                .await
                .map_err(map_err)?;
            }
        }
        SettlementEvent::OrderClosed { order_uid }
        | SettlementEvent::OrderCancelled { order_uid } => {
            sqlx::query(
                "UPDATE solana.order_pda SET cancellation_timestamp = now() WHERE order_uid = $1 \
                 AND cancellation_timestamp IS NULL",
            )
            .bind(order_uid.0.to_vec())
            .execute(&mut *conn)
            .await
            .map_err(map_err)?;
        }
        // No MVP table yet: buffers, manager/solver admin events, generic
        // interactions. Skipped until their tables and PRs land.
        SettlementEvent::BufferCreated { .. }
        | SettlementEvent::BufferUsed { .. }
        | SettlementEvent::ManagerUpdated { .. }
        | SettlementEvent::SolverAdded { .. }
        | SettlementEvent::SolverRemoved { .. }
        | SettlementEvent::SolverInteraction { .. } => {}
    }
    Ok(())
}

fn map_err(error: sqlx::Error) -> PersistenceError {
    match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => PersistenceError::Conflict,
        _ => PersistenceError::Unavailable,
    }
}
