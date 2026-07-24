//! PostgresStore tests against a real docker Postgres.
//!
//! Requires `docker compose up -d` and runs serially (`--test-threads 1`), the
//! same harness the EVM database tests use. The `solana.*` schema comes from
//! the test-only `schema.sql` fixture applied here, not from a committed flyway
//! migration, so nothing lands on staging/prod while the shape is still moving.

use {
    super::Persistence,
    crate::types::{
        events::{DecodedEvent, SettlementEvent, TradeDelta},
        order::OrderUid,
        slot::Slot,
    },
    solana_sdk::{pubkey::Pubkey, signature::Signature},
    sqlx::{PgPool, types::BigDecimal},
};

const SCHEMA: &str = include_str!("schema.sql");

#[tokio::test]
#[ignore = "requires postgres: docker compose up -d, run with --test-threads 1"]
async fn persists_settlement_finalized_and_advances_watermark() {
    let pool = PgPool::connect("postgresql://").await.unwrap();
    // Fresh solana schema each run, committed to the throwaway docker DB and
    // dropped at the end. Never a flyway migration, so staging/prod stay clean.
    sqlx::raw_sql("DROP SCHEMA IF EXISTS solana CASCADE")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(SCHEMA).execute(&pool).await.unwrap();

    let store = Persistence::new(pool.clone());

    let tx_signature = Signature::from([7u8; 64]);
    let order_uid = OrderUid([1u8; 32]);
    let event = DecodedEvent::Settlement(SettlementEvent::SettlementFinalized {
        auction_id: 42,
        solver: Pubkey::from([2u8; 32]),
        tx_signature,
        slot: Slot(100),
        trades: vec![TradeDelta {
            order_uid,
            amount_withdrawn_delta: 1_000,
            amount_received_delta: 900,
            order_fulfilled: true,
        }],
    });

    store.persist_events(vec![event], 100).await.unwrap();

    let (auction_id,): (i64,) =
        sqlx::query_as("SELECT auction_id FROM solana.settlements WHERE tx_signature = $1")
            .bind(tx_signature.as_ref().to_vec())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(auction_id, 42);

    let (sell,): (BigDecimal,) =
        sqlx::query_as("SELECT sell_amount FROM solana.trades WHERE order_uid = $1")
            .bind(order_uid.0.to_vec())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sell, BigDecimal::from(1_000u64));

    assert_eq!(store.read_watermark().await.unwrap(), Some(100));

    // Monotonic: a lower watermark is ignored.
    store.write_watermark(50).await.unwrap();
    assert_eq!(store.read_watermark().await.unwrap(), Some(100));

    sqlx::raw_sql("DROP SCHEMA solana CASCADE")
        .execute(&pool)
        .await
        .unwrap();
}
