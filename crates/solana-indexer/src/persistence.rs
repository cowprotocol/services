//! PostgreSQL persistence layer for decoded events and slot state.

use {
    crate::types::{
        Signature,
        errors::PersistenceError,
        events::{
            CreatedOrder,
            DecodedEvent,
            FinalizedSettlement,
            OrderKind,
            SettlementEvent,
            TradeDelta,
        },
        slot::Slot,
    },
    bigdecimal::BigDecimal,
    database::solana::OrderEventLabel,
    solana_sdk::pubkey::Pubkey,
    sqlx::{PgPool, PgTransaction},
    std::collections::HashMap,
};

/// Slots stay far below `i64::MAX`, the conversion to the database's
/// `bigint` is lossless.
fn to_db_slot(slot: Slot) -> i64 {
    i64::try_from(u64::from(slot)).expect("slot exceeds i64")
}

/// A transaction holds far fewer instructions than `i32::MAX`.
fn to_db_instruction_index(index: u32) -> i32 {
    i32::try_from(index).expect("instruction index exceeds i32")
}

/// The database never stores a negative slot, `to_db_slot` is the only
/// writer.
fn from_db_slot(slot: i64) -> Slot {
    Slot(u64::try_from(slot).expect("negative slot in the database"))
}

/// A `solana.orders.uid` value, 32 bytes by schema constraint.
fn from_db_uid(uid: Vec<u8>) -> [u8; 32] {
    uid.try_into().expect("uid length enforced by the schema")
}

/// Why a transaction sits in `solana.dead_letter`, stored as the `reason`
/// column value the replay tooling filters on.
enum DeadLetterReason {
    /// The transaction failed to decode.
    DecoderError,
    /// A created order's token accounts did not resolve to mints.
    UnresolvedMints,
    /// A settlement trade named an order PDA with no orders row.
    UnresolvedOrders,
}

impl DeadLetterReason {
    fn as_str(&self) -> &'static str {
        match self {
            Self::DecoderError => "decoder_error",
            Self::UnresolvedMints => "unresolved_mints",
            Self::UnresolvedOrders => "unresolved_orders",
        }
    }
}

/// Postgres implementation over the `solana.*` schema.
#[derive(Clone)]
pub(crate) struct Postgres {
    pool: PgPool,
}

impl Postgres {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The last fully indexed slot, the stream resumes one past it. `None`
    /// before the first write.
    pub(crate) async fn last_indexed_slot(&self) -> Result<Option<Slot>, PersistenceError> {
        let slot: Option<i64> = sqlx::query_scalar("SELECT slot FROM solana.indexer_state")
            .fetch_optional(&self.pool)
            .await?;
        Ok(slot.map(from_db_slot))
    }

    async fn apply(
        tx: &mut PgTransaction<'_>,
        event: DecodedEvent,
        mints: &HashMap<Pubkey, Pubkey>,
        slot: Slot,
    ) -> Result<(), PersistenceError> {
        match event {
            DecodedEvent::Settlement(SettlementEvent::OrderCreated(order)) => {
                Self::apply_order_created(tx, &order, mints, slot).await
            }
            DecodedEvent::Settlement(SettlementEvent::SettlementFinalized(settlement)) => {
                Self::apply_settlement_finalized(tx, settlement).await
            }
            DecodedEvent::Settlement(other) => {
                tracing::debug!(event = ?other, "settlement event without a persistence mapping");
                Ok(())
            }
            DecodedEvent::SolFlow(event) => {
                tracing::debug!(event = ?event, "solflow event without a persistence mapping");
                Ok(())
            }
        }
    }

    async fn apply_order_created(
        tx: &mut PgTransaction<'_>,
        order: &CreatedOrder,
        mints: &HashMap<Pubkey, Pubkey>,
        slot: Slot,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"
INSERT INTO solana.order_pda (order_uid, created_by)
VALUES ($1, $2)
ON CONFLICT (order_uid) DO NOTHING
            "#,
        )
        .bind(order.order_uid.0)
        .bind(order.created_by.to_bytes())
        .execute(&mut **tx)
        .await?;
        // Without both mints the intent row cannot be written. The
        // transaction is dead-lettered so the replay machinery re-delivers
        // it, until then the order stays out of the solvable set.
        let (Some(sell_token), Some(buy_token)) = (
            mints.get(&order.sell_token_account),
            mints.get(&order.buy_token_account),
        ) else {
            tracing::warn!(
                order_uid = %order.order_uid,
                "unresolved token account mints, order dead-lettered"
            );
            Self::insert_dead_letter(
                &mut **tx,
                order.signature,
                slot,
                DeadLetterReason::UnresolvedMints,
            )
            .await?;
            return Ok(());
        };
        // creation_timestamp is the indexing time, the stream carries no
        // block time.
        let inserted = sqlx::query(
            r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), $13)
ON CONFLICT (uid) DO NOTHING
            "#,
        )
        .bind(order.order_uid.0)
        .bind(order.owner.to_bytes())
        .bind(sell_token.to_bytes())
        .bind(buy_token.to_bytes())
        .bind(order.sell_token_account.to_bytes())
        .bind(order.buy_token_account.to_bytes())
        .bind(BigDecimal::from(order.sell_amount))
        .bind(BigDecimal::from(order.buy_amount))
        .bind(i64::from(order.valid_to))
        .bind(match order.kind {
            OrderKind::Sell => database::solana::OrderKind::Sell,
            OrderKind::Buy => database::solana::OrderKind::Buy,
        })
        .bind(order.partially_fillable)
        .bind(order.app_data.to_vec())
        .bind(order.order_pda.to_bytes())
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if inserted > 0 {
            Self::insert_order_event(tx, order.order_uid.0, OrderEventLabel::Created).await?;
        }
        Ok(())
    }

    /// Append one auction-progress event, in the caller's transaction so the
    /// event lands atomically with the row that caused it.
    async fn insert_order_event(
        tx: &mut PgTransaction<'_>,
        order_uid: [u8; 32],
        label: OrderEventLabel,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            "INSERT INTO solana.order_events (order_uid, timestamp, label) VALUES ($1, now(), $2)",
        )
        .bind(order_uid)
        .bind(label)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn apply_settlement_finalized(
        tx: &mut PgTransaction<'_>,
        settlement: FinalizedSettlement,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"
INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, auction_id, solution_uid)
VALUES ($1, $2, $3, $4, $5, NULL)
ON CONFLICT (tx_signature, instruction_index) DO NOTHING
            "#,
        )
        .bind(to_db_slot(settlement.slot))
        .bind(settlement.tx_signature.as_ref())
        .bind(to_db_instruction_index(settlement.instruction_index))
        .bind(settlement.solver.to_bytes())
        .bind(settlement.auction_id)
        .execute(&mut **tx)
        .await?;
        for trade in &settlement.trades {
            Self::apply_trade(tx, &settlement, *trade).await?;
        }
        Ok(())
    }

    /// Insert one trade row and, only when the row is new, fold its deltas
    /// into the order PDA's running sums. The conflict check keys the sums to
    /// the insert so a replayed settlement cannot double-apply them.
    ///
    /// The trade names its order by PDA, the orders table maps it to the UID.
    /// An unknown PDA dead-letters the transaction: the order's own creation
    /// was dead-lettered (or never indexed), so the replay restores the order
    /// row first and this settlement's trade on the second pass.
    async fn apply_trade(
        tx: &mut PgTransaction<'_>,
        settlement: &FinalizedSettlement,
        trade: TradeDelta,
    ) -> Result<(), PersistenceError> {
        let order_uid: Option<[u8; 32]> =
            sqlx::query_scalar("SELECT uid FROM solana.orders WHERE order_pda = $1")
                .bind(trade.order_pda.to_bytes())
                .fetch_optional(&mut **tx)
                .await?
                .map(from_db_uid);
        let Some(order_uid) = order_uid else {
            tracing::warn!(
                order_pda = %trade.order_pda,
                "trade for an unknown order PDA, settlement dead-lettered"
            );
            Self::insert_dead_letter(
                &mut **tx,
                settlement.tx_signature,
                settlement.slot,
                DeadLetterReason::UnresolvedOrders,
            )
            .await?;
            return Ok(());
        };
        // The fee is not on-chain data (it comes from the off-chain solution),
        // so the column holds zero.
        let inserted = sqlx::query(
            r#"
INSERT INTO solana.trades (tx_signature, instruction_index, order_uid, sell_amount,
    buy_amount, fee_amount)
VALUES ($1, $2, $3, $4, $5, 0)
ON CONFLICT DO NOTHING
            "#,
        )
        .bind(settlement.tx_signature.as_ref())
        .bind(to_db_instruction_index(settlement.instruction_index))
        .bind(order_uid)
        .bind(BigDecimal::from(trade.amount_withdrawn_delta))
        .bind(BigDecimal::from(trade.amount_received_delta))
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if inserted == 0 {
            return Ok(());
        }
        Self::insert_order_event(tx, order_uid, OrderEventLabel::Traded).await?;
        let updated = sqlx::query(
            r#"
UPDATE solana.order_pda
SET amount_withdrawn = amount_withdrawn + $2,
    amount_received = amount_received + $3
WHERE order_uid = $1
            "#,
        )
        .bind(order_uid)
        .bind(BigDecimal::from(trade.amount_withdrawn_delta))
        .bind(BigDecimal::from(trade.amount_received_delta))
        .execute(&mut **tx)
        .await?
        .rows_affected();
        if updated == 0 {
            // Unreachable through the indexer's own writes (an orders row is
            // only ever written after its order_pda row), so this guards
            // manually repaired databases. The trade row keeps the deltas,
            // so the sums are reconstructible, but nothing applies them
            // automatically.
            tracing::warn!(
                order_pda = %trade.order_pda,
                "trade for an order without an order_pda row, sums not applied"
            );
        }
        Ok(())
    }

    /// Upsert the last indexed slot, ignoring backward writes so the table's
    /// monotone trigger never fires.
    async fn upsert_last_indexed_slot(
        ex: impl sqlx::PgExecutor<'_>,
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
        .execute(ex)
        .await?;
        Ok(())
    }

    /// Save one slot's decoded events and advance the last indexed slot in
    /// one transaction. `mints` maps the batch's token accounts to their
    /// mints.
    pub(crate) async fn persist_events(
        &self,
        events: Vec<DecodedEvent>,
        mints: &HashMap<Pubkey, Pubkey>,
        last_indexed_slot: Slot,
    ) -> Result<(), PersistenceError> {
        let mut tx = self.pool.begin().await?;
        for event in events {
            Self::apply(&mut tx, event, mints, last_indexed_slot).await?;
        }
        Self::upsert_last_indexed_slot(&mut *tx, last_indexed_slot).await?;
        Ok(tx.commit().await?)
    }

    /// Advance the finalized watermark. Update-only: before the first flush
    /// there is no state row and nothing indexed to finalize. A backward
    /// write is a no-op.
    pub(crate) async fn write_finalized_slot(&self, slot: Slot) -> Result<(), PersistenceError> {
        sqlx::query(
            "UPDATE solana.indexer_state SET finalized_slot = GREATEST(finalized_slot, $1)",
        )
        .bind(to_db_slot(slot))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a slot as fully indexed. A backward write is a no-op.
    pub(crate) async fn write_last_indexed_slot(&self, slot: Slot) -> Result<(), PersistenceError> {
        Self::upsert_last_indexed_slot(&self.pool, slot).await
    }

    /// Record a transaction whose decode failed so recovery can replay it by
    /// signature.
    pub(crate) async fn record_decode_failure(
        &self,
        signature: Signature,
        slot: Slot,
    ) -> Result<(), PersistenceError> {
        Self::insert_dead_letter(&self.pool, signature, slot, DeadLetterReason::DecoderError).await
    }

    /// Mark a transaction for replay by signature. One row per transaction,
    /// idempotent on the signature, so the first recorded reason wins.
    async fn insert_dead_letter(
        ex: impl sqlx::PgExecutor<'_>,
        signature: Signature,
        slot: Slot,
        reason: DeadLetterReason,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"
INSERT INTO solana.dead_letter (slot, tx_signature, reason)
VALUES ($1, $2, $3)
ON CONFLICT (tx_signature) DO NOTHING
            "#,
        )
        .bind(to_db_slot(slot))
        .bind(signature.as_ref().to_vec())
        .bind(reason.as_str())
        .execute(ex)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::Postgres,
        crate::{
            test_db::{pool, wipe},
            types::{
                Signature,
                events::{DecodedEvent, FinalizedSettlement, SettlementEvent, TradeDelta},
                order::OrderUid,
                slot::Slot,
            },
        },
        bigdecimal::BigDecimal,
        solana_sdk::pubkey::Pubkey,
        sqlx::{PgPool, Row},
        std::collections::HashMap,
    };

    /// The finalized watermark only moves forward and needs an existing
    /// state row: before the first flush the update is a no-op.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_finalized_watermark_is_monotone_and_update_only() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        // No state row yet: the write lands nowhere.
        postgres.write_finalized_slot(Slot(5)).await.unwrap();
        let row: Option<i64> =
            sqlx::query_scalar("SELECT finalized_slot FROM solana.indexer_state")
                .fetch_optional(&pool)
                .await
                .unwrap();
        assert_eq!(row, None);

        postgres.write_last_indexed_slot(Slot(10)).await.unwrap();
        postgres.write_finalized_slot(Slot(8)).await.unwrap();
        postgres.write_finalized_slot(Slot(6)).await.unwrap();
        let finalized: i64 = sqlx::query_scalar("SELECT finalized_slot FROM solana.indexer_state")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(finalized, 8);
    }

    /// The `solana.orders` columns a seeded test order writes.
    struct SeedOrder {
        uid: [u8; 32],
        owner: [u8; 32],
        sell_amount: i64,
        buy_amount: i64,
        valid_to: i64,
        kind: database::solana::OrderKind,
        order_pda: [u8; 32],
    }

    impl SeedOrder {
        fn new(uid: [u8; 32]) -> Self {
            Self {
                uid,
                owner: [0xAA; 32],
                sell_amount: 1_000,
                buy_amount: 2_000,
                valid_to: 42,
                kind: database::solana::OrderKind::Sell,
                order_pda: uid,
            }
        }

        async fn insert(self, pool: &PgPool) {
            sqlx::query(
                r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda)
VALUES ($1, $2, $2, $2, $2, $2, $3, $4, $5, $6, false, $2, now(), $7)
                "#,
            )
            .bind(self.uid)
            .bind(self.owner)
            .bind(self.sell_amount)
            .bind(self.buy_amount)
            .bind(self.valid_to)
            .bind(self.kind)
            .bind(self.order_pda)
            .execute(pool)
            .await
            .unwrap();
        }
    }

    fn created_order(uid: [u8; 32]) -> crate::types::events::CreatedOrder {
        crate::types::events::CreatedOrder {
            signature: Signature::from([6; 64]),
            order_uid: OrderUid(uid),
            owner: Pubkey::new_from_array([0xAA; 32]),
            created_by: Pubkey::new_from_array([0xBB; 32]),
            order_pda: Pubkey::new_from_array([0xDD; 32]),
            sell_token_account: Pubkey::new_from_array([4; 32]),
            buy_token_account: Pubkey::new_from_array([5; 32]),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: 42,
            kind: crate::types::events::OrderKind::Sell,
            partially_fillable: false,
            app_data: [0; 32],
        }
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_unresolved_mints_skip_the_orders_row() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        let uid = [0x77; 32];
        let events = vec![DecodedEvent::Settlement(SettlementEvent::OrderCreated(
            Box::new(created_order(uid)),
        ))];
        postgres
            .persist_events(events, &HashMap::new(), Slot(30))
            .await
            .unwrap();

        let pda: i64 =
            sqlx::query_scalar("SELECT count(*) FROM solana.order_pda WHERE order_uid = $1")
                .bind(uid)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pda, 1);
        let orders: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.orders WHERE uid = $1")
            .bind(uid)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(orders, 0);
        let reason: String =
            sqlx::query_scalar("SELECT reason FROM solana.dead_letter WHERE tx_signature = $1")
                .bind(Signature::from([6; 64]).as_ref())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reason, "unresolved_mints");
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_last_indexed_slot_upserts_forward_and_ignores_backward() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool);

        assert_eq!(postgres.last_indexed_slot().await.unwrap(), None);
        postgres.write_last_indexed_slot(Slot(10)).await.unwrap();
        assert_eq!(postgres.last_indexed_slot().await.unwrap(), Some(Slot(10)));
        postgres.write_last_indexed_slot(Slot(7)).await.unwrap();
        assert_eq!(postgres.last_indexed_slot().await.unwrap(), Some(Slot(10)));
        postgres.write_last_indexed_slot(Slot(11)).await.unwrap();
        assert_eq!(postgres.last_indexed_slot().await.unwrap(), Some(Slot(11)));
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_dead_letter_is_idempotent_on_the_signature() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        let signature = Signature::from([7; 64]);
        postgres
            .record_decode_failure(signature, Slot(5))
            .await
            .unwrap();
        postgres
            .record_decode_failure(signature, Slot(6))
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
    async fn solana_db_persist_events_writes_the_batch_once() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        let uid = [1_u8; 32];
        SeedOrder::new(uid).insert(&pool).await;
        // A second order with an unseeded uid: its orders row comes from the
        // event itself, the seeded uid keeps the seed row via ON CONFLICT.
        let fresh_uid = [2_u8; 32];
        let events = vec![
            DecodedEvent::Settlement(SettlementEvent::OrderCreated(Box::new(created_order(uid)))),
            DecodedEvent::Settlement(SettlementEvent::OrderCreated(Box::new(created_order(
                fresh_uid,
            )))),
            DecodedEvent::Settlement(SettlementEvent::SettlementFinalized(FinalizedSettlement {
                auction_id: 77,
                solver: Pubkey::new_from_array([0xCC; 32]),
                tx_signature: Signature::from([9; 64]),
                slot: Slot(20),
                instruction_index: 1,
                trades: vec![TradeDelta {
                    // `SeedOrder` writes the uid as its own order PDA.
                    order_pda: Pubkey::new_from_array(uid),
                    amount_withdrawn_delta: 300,
                    amount_received_delta: 500,
                }],
            })),
            // A second settlement in the same transaction keeps its own row.
            DecodedEvent::Settlement(SettlementEvent::SettlementFinalized(FinalizedSettlement {
                auction_id: 78,
                solver: Pubkey::new_from_array([0xCC; 32]),
                tx_signature: Signature::from([9; 64]),
                slot: Slot(20),
                instruction_index: 3,
                trades: vec![],
            })),
        ];

        let mints = HashMap::from([
            (
                Pubkey::new_from_array([4; 32]),
                Pubkey::new_from_array([0xA1; 32]),
            ),
            (
                Pubkey::new_from_array([5; 32]),
                Pubkey::new_from_array([0xA2; 32]),
            ),
        ]);

        // Twice: the second run must change nothing, replay is idempotent.
        postgres
            .persist_events(events.clone(), &mints, Slot(20))
            .await
            .unwrap();
        postgres
            .persist_events(events, &mints, Slot(20))
            .await
            .unwrap();

        let created = sqlx::query(
            "SELECT sell_token, buy_token, sell_amount FROM solana.orders WHERE uid = $1",
        )
        .bind(fresh_uid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(created.get::<Vec<u8>, _>("sell_token"), vec![0xA1; 32]);
        assert_eq!(created.get::<Vec<u8>, _>("buy_token"), vec![0xA2; 32]);

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
        // The trade names the order PDA, the row must carry the resolved uid.
        let trade_uids: Vec<Vec<u8>> = sqlx::query_scalar("SELECT order_uid FROM solana.trades")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!((settlements, trade_uids), (2, vec![uid.to_vec()]));
        assert_eq!(postgres.last_indexed_slot().await.unwrap(), Some(Slot(20)));

        // One event per actually-inserted row, stable under the replay: the
        // fresh order was created, the seeded order (whose insert the
        // conflict skipped) only traded.
        let events: Vec<(Vec<u8>, String)> =
            sqlx::query_as("SELECT order_uid, label::text FROM solana.order_events ORDER BY label")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            events,
            vec![
                (fresh_uid.to_vec(), "created".to_string()),
                (uid.to_vec(), "traded".to_string()),
            ]
        );
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied locally, run with --test-threads 1"]
    async fn solana_db_unknown_order_pda_dead_letters_the_settlement() {
        let pool = pool().await;
        wipe(&pool).await;
        let postgres = Postgres::new(pool.clone());

        let events = vec![DecodedEvent::Settlement(
            SettlementEvent::SettlementFinalized(FinalizedSettlement {
                auction_id: 77,
                solver: Pubkey::new_from_array([0xCC; 32]),
                tx_signature: Signature::from([9; 64]),
                slot: Slot(20),
                instruction_index: 1,
                trades: vec![TradeDelta {
                    order_pda: Pubkey::new_from_array([0xEE; 32]),
                    amount_withdrawn_delta: 300,
                    amount_received_delta: 500,
                }],
            }),
        )];
        postgres
            .persist_events(events, &HashMap::new(), Slot(20))
            .await
            .unwrap();

        // The settlement row lands, the unresolvable trade is replaced by a
        // replay marker.
        let settlements: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.settlements")
            .fetch_one(&pool)
            .await
            .unwrap();
        let trades: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.trades")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!((settlements, trades), (1, 0));
        let reason: String =
            sqlx::query_scalar("SELECT reason FROM solana.dead_letter WHERE tx_signature = $1")
                .bind(Signature::from([9; 64]).as_ref())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(reason, "unresolved_orders");
    }
}
