//! Read access to the `solana.*` tables the indexer writes.
//!
//! Runtime SQL, no compile-time macros: the schema lives on a separate
//! migration branch and is applied out of band, so queries are checked when
//! the ignored DB tests run against it, not at build time.

use {
    anyhow::{Context, Result},
    database::{
        byte_array::ByteArray,
        orders::{OrderClass, OrderKind},
    },
    sqlx::{
        PgExecutor,
        types::{
            BigDecimal,
            chrono::{DateTime, Utc},
        },
    },
};

/// A row of `solana.orders`. The row is written off-chain by the orderbook,
/// the autopilot only reads it.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Order {
    pub uid: ByteArray<32>,
    pub owner: ByteArray<32>,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_token_account: ByteArray<32>,
    pub buy_token_account: ByteArray<32>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub valid_to: i64,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub app_data: ByteArray<32>,
    pub intent_signature: Option<ByteArray<64>>,
    pub creation_timestamp: DateTime<Utc>,
    pub class: OrderClass,
    pub order_pda: ByteArray<32>,
}

/// A row of `solana.settlements`, the indexer's record of one settlement
/// transaction. `solution_uid` is filled in later by the indexer, so it is
/// nullable.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Settlement {
    pub slot: i64,
    pub tx_signature: ByteArray<64>,
    pub solver: ByteArray<32>,
    pub auction_id: i64,
    pub solution_uid: Option<i64>,
    pub commitment: String,
}

/// Latest slot the indexer has processed at `confirmed`. `None` before the
/// indexer's first write.
pub async fn slot_watermark(ex: impl PgExecutor<'_>) -> Result<Option<i64>> {
    const QUERY: &str = "SELECT last_indexed_slot FROM solana.indexer_state WHERE id = 0";
    sqlx::query_scalar(QUERY)
        .fetch_optional(ex)
        .await
        .context("read solana.indexer_state watermark")
}

/// Every row of `solana.orders`. Filtering into a solvable set is the caller's
/// job.
pub async fn orders(ex: impl PgExecutor<'_>) -> Result<Vec<Order>> {
    const QUERY: &str = "SELECT uid, owner, sell_token, buy_token, sell_token_account, \
                         buy_token_account, sell_amount, buy_amount, fee_amount, valid_to, kind, \
                         partially_fillable, app_data, intent_signature, creation_timestamp, \
                         class, order_pda FROM solana.orders";
    sqlx::query_as(QUERY)
        .fetch_all(ex)
        .await
        .context("read solana.orders")
}

/// Settlements the indexer recorded for an auction. More than one row when the
/// auction had several winners.
pub async fn settlements_by_auction(
    ex: impl PgExecutor<'_>,
    auction_id: i64,
) -> Result<Vec<Settlement>> {
    const QUERY: &str = "SELECT slot, tx_signature, solver, auction_id, solution_uid, commitment \
                         FROM solana.settlements WHERE auction_id = $1";
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_all(ex)
        .await
        .context("read solana.settlements by auction")
}

#[cfg(test)]
mod tests {
    use {
        super::{orders, settlements_by_auction, slot_watermark},
        database::{
            byte_array::ByteArray,
            orders::{OrderClass, OrderKind},
        },
        sqlx::{PgPool, types::BigDecimal},
    };

    // Inserts one row into each table and reads it back. Runs inside a
    // transaction that rolls back, so it leaves no residue. Needs the
    // `solana.*` schema applied out of band (the migration branch).
    #[tokio::test]
    #[ignore = "needs the solana.* schema, run manually against the migration branch"]
    async fn reads_round_trip() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let mut tx = pool.begin().await.unwrap();

        sqlx::query(
            "INSERT INTO solana.indexer_state (id, last_indexed_slot) VALUES (0, 42) ON CONFLICT \
             (id) DO UPDATE SET last_indexed_slot = EXCLUDED.last_indexed_slot",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(slot_watermark(&mut *tx).await.unwrap(), Some(42));

        sqlx::query(
            "INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account, \
             buy_token_account, sell_amount, buy_amount, fee_amount, valid_to, kind, \
             partially_fillable, app_data, intent_signature, creation_timestamp, class, \
             order_pda) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
             now(), $15, $16)",
        )
        .bind(ByteArray([1u8; 32]))
        .bind(ByteArray([2u8; 32]))
        .bind(ByteArray([3u8; 32]))
        .bind(ByteArray([4u8; 32]))
        .bind(ByteArray([5u8; 32]))
        .bind(ByteArray([6u8; 32]))
        .bind(BigDecimal::from(1000))
        .bind(BigDecimal::from(900))
        .bind(BigDecimal::from(1))
        .bind(1_000_000i64)
        .bind(OrderKind::Sell)
        .bind(false)
        .bind(ByteArray([7u8; 32]))
        .bind(Option::<ByteArray<64>>::None)
        .bind(OrderClass::Limit)
        .bind(ByteArray([8u8; 32]))
        .execute(&mut *tx)
        .await
        .unwrap();
        let orders = orders(&mut *tx).await.unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].uid, ByteArray([1u8; 32]));
        assert_eq!(orders[0].kind, OrderKind::Sell);
        assert_eq!(orders[0].class, OrderClass::Limit);
        assert_eq!(orders[0].intent_signature, None);

        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, solver, auction_id, \
             solution_uid, commitment) VALUES (7, $1, $2, 123, NULL, 'finalized')",
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray([10u8; 32]))
        .execute(&mut *tx)
        .await
        .unwrap();
        let settlements = settlements_by_auction(&mut *tx, 123).await.unwrap();
        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].auction_id, 123);
        assert_eq!(settlements[0].solution_uid, None);
        assert_eq!(settlements[0].commitment, "finalized");
    }
}
