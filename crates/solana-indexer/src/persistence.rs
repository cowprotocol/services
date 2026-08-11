//! PostgreSQL persistence layer for decoded events and slot state.

use {
    crate::types::{
        Signature,
        errors::PersistenceError,
        events::{DecodedEvent, SettlementEvent, TradeDelta},
        slot::Slot,
    },
    async_trait::async_trait,
    bigdecimal::BigDecimal,
    sqlx::{PgPool, PgTransaction},
};

/// The decoder's persistence seam. The Postgres implementation is the one
/// production backend, tests drive the decoder through a recording double.
#[async_trait]
pub(crate) trait Persistence: Send + Sync {
    /// Save one slot's decoded events and advance the slot watermark in one
    /// transaction.
    async fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        new_watermark: Slot,
    ) -> Result<(), PersistenceError>;

    /// Record a slot checkpoint. A backward write is a no-op.
    async fn write_watermark(&self, slot: Slot) -> Result<(), PersistenceError>;

    /// Record a transaction whose decode failed so recovery can replay it by
    /// signature. One row per transaction, idempotent on the signature.
    async fn write_dead_letter(
        &self,
        signature: Signature,
        slot: Slot,
    ) -> Result<(), PersistenceError>;
}

/// Slots stay far below `i64::MAX`, the cast to the database's `bigint` is
/// lossless.
fn to_db_slot(slot: Slot) -> i64 {
    i64::try_from(u64::from(slot)).expect("slot exceeds i64")
}

/// Postgres implementation over the `solana.*` schema.
#[derive(Clone)]
pub(crate) struct Postgres {
    pool: PgPool,
}

impl Postgres {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "constructed by the binary wiring")
    )]
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The watermark the stream resumes from after a restart. `None` before
    /// the first write.
    pub(crate) async fn read_watermark(&self) -> Result<Option<Slot>, PersistenceError> {
        let slot: Option<i64> = sqlx::query_scalar("SELECT slot FROM solana.indexer_state")
            .fetch_optional(&self.pool)
            .await?;
        Ok(slot.map(|slot| Slot(slot as u64)))
    }

    async fn apply(
        tx: &mut PgTransaction<'_>,
        event: DecodedEvent,
    ) -> Result<(), PersistenceError> {
        match event {
            DecodedEvent::Settlement(SettlementEvent::OrderCreated {
                order_uid,
                created_by,
                ..
            }) => {
                // The order row itself is written by the orderbook at intake.
                // TODO: insert `solana.orders` here too for orders created
                // directly on chain, which needs the intent fields on the
                // event.
                sqlx::query(
                    r#"
INSERT INTO solana.order_pda (order_uid, created_by)
VALUES ($1, $2)
ON CONFLICT (order_uid) DO NOTHING
                    "#,
                )
                .bind(order_uid.0)
                .bind(created_by.to_bytes())
                .execute(&mut **tx)
                .await?;
            }
            DecodedEvent::Settlement(SettlementEvent::SettlementFinalized {
                auction_id,
                solver,
                tx_signature,
                slot,
                instruction_index,
                inner_ix_path,
                trades,
            }) => {
                sqlx::query(
                    r#"
INSERT INTO solana.settlements (slot, tx_signature, solver, auction_id, solution_uid)
VALUES ($1, $2, $3, $4, NULL)
ON CONFLICT (tx_signature) DO NOTHING
                    "#,
                )
                .bind(to_db_slot(slot))
                .bind(tx_signature.as_ref())
                .bind(solver.to_bytes())
                .bind(auction_id)
                .execute(&mut **tx)
                .await?;
                let path: Vec<i32> = inner_ix_path.iter().map(|&step| i32::from(step)).collect();
                for trade in trades {
                    Self::apply_trade(tx, tx_signature, instruction_index, &path, trade).await?;
                }
            }
            DecodedEvent::Settlement(other) => {
                // No table maps these yet (buffers are informational, the
                // state-PDA mirror and order lifecycle arrive with their
                // decoder support).
                tracing::debug!(event = ?other, "settlement event without a persistence mapping");
            }
            DecodedEvent::SolFlow(event) => {
                // The SolFlow program is post-launch scoped.
                tracing::debug!(event = ?event, "solflow event without a persistence mapping");
            }
        }
        Ok(())
    }

    /// Insert one trade row and, only when the row is new, fold its deltas
    /// into the order PDA's running sums. The conflict check keys the sums to
    /// the insert so a replayed settlement cannot double-apply them.
    async fn apply_trade(
        tx: &mut PgTransaction<'_>,
        tx_signature: Signature,
        instruction_index: u32,
        inner_ix_path: &[i32],
        trade: TradeDelta,
    ) -> Result<(), PersistenceError> {
        // The fee is not on-chain data, it arrives from the off-chain solution
        // and is reconciled by the autopilot. Zero until that wiring exists.
        let inserted = sqlx::query(
            r#"
INSERT INTO solana.trades (settlement_tx_signature, instruction_index, inner_ix_path,
    order_uid, sell_amount, buy_amount, fee_amount)
VALUES ($1, $2, $3, $4, $5, $6, 0)
ON CONFLICT DO NOTHING
            "#,
        )
        .bind(tx_signature.as_ref())
        .bind(i32::try_from(instruction_index).expect("instruction index fits i32"))
        .bind(inner_ix_path)
        .bind(trade.order_uid.0)
        .bind(BigDecimal::from(trade.amount_withdrawn_delta))
        .bind(BigDecimal::from(trade.amount_received_delta))
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            return Ok(());
        }
        sqlx::query(
            r#"
UPDATE solana.order_pda
SET amount_withdrawn = amount_withdrawn + $2,
    amount_received = amount_received + $3
WHERE order_uid = $1
            "#,
        )
        .bind(trade.order_uid.0)
        .bind(BigDecimal::from(trade.amount_withdrawn_delta))
        .bind(BigDecimal::from(trade.amount_received_delta))
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// Upsert the watermark, ignoring backward writes so the table's monotone
    /// trigger never fires.
    async fn upsert_watermark(
        tx: &mut PgTransaction<'_>,
        slot: Slot,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"
INSERT INTO solana.indexer_state (slot)
VALUES ($1)
ON CONFLICT (singleton) DO UPDATE SET slot = EXCLUDED.slot
WHERE indexer_state.slot < EXCLUDED.slot
            "#,
        )
        .bind(to_db_slot(slot))
        .execute(&mut **tx)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl Persistence for Postgres {
    async fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        new_watermark: Slot,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool.begin().await?;
        for event in events {
            Self::apply(&mut tx, event).await?;
        }
        Self::upsert_watermark(&mut tx, new_watermark).await?;
        Ok(tx.commit().await?)
    }

    async fn write_watermark(&self, slot: Slot) -> Result<(), PersistenceError> {
        let mut tx = self.pool.begin().await?;
        Self::upsert_watermark(&mut tx, slot).await?;
        Ok(tx.commit().await?)
    }

    async fn write_dead_letter(
        &self,
        signature: Signature,
        slot: Slot,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"
INSERT INTO solana.dead_letter (slot, tx_signature, reason)
VALUES ($1, $2, 'decoder_error')
ON CONFLICT (tx_signature) DO NOTHING
            "#,
        )
        .bind(to_db_slot(slot))
        .bind(signature.as_ref())
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::{Persistence, Postgres},
        crate::types::{
            Signature,
            events::{DecodedEvent, SettlementEvent, TradeDelta},
            order::OrderUid,
            slot::Slot,
        },
        bigdecimal::BigDecimal,
        solana_sdk::pubkey::Pubkey,
        sqlx::{PgPool, Row},
    };

    async fn pool() -> PgPool {
        PgPool::connect("postgresql://").await.unwrap()
    }

    async fn wipe(pool: &PgPool) {
        for table in [
            "solana.trades",
            "solana.settlements",
            "solana.order_pda",
            "solana.orders",
            "solana.dead_letter",
            "solana.indexer_state",
        ] {
            sqlx::query(&format!("DELETE FROM {table}"))
                .execute(pool)
                .await
                .unwrap();
        }
    }

    async fn seed_order(pool: &PgPool, uid: [u8; 32]) {
        sqlx::query(
            r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, fee_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, class, order_pda)
VALUES ($1, $2, $2, $2, $2, $2, 1000, 2000, 0, 42, 'sell', false, $2, now(), 'market', $1)
            "#,
        )
        .bind(uid)
        .bind([0xAA_u8; 32])
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn watermark_upserts_forward_and_ignores_backward() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool);

        assert_eq!(postgres.read_watermark().await.unwrap(), None);
        postgres.write_watermark(Slot(10)).await.unwrap();
        assert_eq!(postgres.read_watermark().await.unwrap(), Some(Slot(10)));
        postgres.write_watermark(Slot(7)).await.unwrap();
        assert_eq!(postgres.read_watermark().await.unwrap(), Some(Slot(10)));
        postgres.write_watermark(Slot(11)).await.unwrap();
        assert_eq!(postgres.read_watermark().await.unwrap(), Some(Slot(11)));
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn dead_letter_is_idempotent_on_the_signature() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        let signature = Signature::from([7; 64]);
        postgres
            .write_dead_letter(signature, Slot(5))
            .await
            .unwrap();
        postgres
            .write_dead_letter(signature, Slot(6))
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.dead_letter")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn persist_events_writes_the_batch_once() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        let uid = [1_u8; 32];
        seed_order(&pool, uid).await;
        let events = vec![
            DecodedEvent::Settlement(SettlementEvent::OrderCreated {
                order_uid: OrderUid(uid),
                owner: Pubkey::new_from_array([0xAA; 32]),
                created_by: Pubkey::new_from_array([0xBB; 32]),
            }),
            DecodedEvent::Settlement(SettlementEvent::SettlementFinalized {
                auction_id: 77,
                solver: Pubkey::new_from_array([0xCC; 32]),
                tx_signature: Signature::from([9; 64]),
                slot: Slot(20),
                instruction_index: 1,
                inner_ix_path: vec![],
                trades: vec![TradeDelta {
                    order_uid: OrderUid(uid),
                    amount_withdrawn_delta: 300,
                    amount_received_delta: 500,
                    order_fulfilled: false,
                }],
            }),
        ];

        // Twice: the second run must change nothing, replay is idempotent.
        postgres
            .persist_events(events.clone(), Slot(20))
            .await
            .unwrap();
        postgres.persist_events(events, Slot(20)).await.unwrap();

        let pda = sqlx::query(
            "SELECT created_by, amount_withdrawn, amount_received FROM solana.order_pda WHERE \
             order_uid = $1",
        )
        .bind(uid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(pda.get::<Vec<u8>, _>("created_by"), vec![0xBB; 32]);
        assert_eq!(
            pda.get::<BigDecimal, _>("amount_withdrawn"),
            BigDecimal::from(300u64)
        );
        assert_eq!(
            pda.get::<BigDecimal, _>("amount_received"),
            BigDecimal::from(500u64)
        );

        let settlements: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.settlements")
            .fetch_one(&pool)
            .await
            .unwrap();
        let trades: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.trades")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((settlements, trades), (1, 1));
        assert_eq!(postgres.read_watermark().await.unwrap(), Some(Slot(20)));
    }
}
