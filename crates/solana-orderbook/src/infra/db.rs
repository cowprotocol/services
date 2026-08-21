//! Database access for the Solana orderbook.

use {
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    chrono::{DateTime, Utc},
    database::byte_array::ByteArray,
    sqlx::PgExecutor,
};

/// One order joined with its fill state.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct OrderRow {
    pub uid: ByteArray<32>,
    pub owner: ByteArray<32>,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_token_account: ByteArray<32>,
    pub buy_token_account: ByteArray<32>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub valid_to: i64,
    pub kind: String,
    pub partially_fillable: bool,
    pub app_data: ByteArray<32>,
    pub creation_timestamp: DateTime<Utc>,
    pub order_pda: ByteArray<32>,
    pub amount_withdrawn: BigDecimal,
    pub amount_received: BigDecimal,
    pub cancellation_timestamp: Option<DateTime<Utc>>,
}

/// Read one order with its fill state. `None` when the uid is unknown.
pub async fn order_by_uid(ex: impl PgExecutor<'_>, uid: [u8; 32]) -> Result<Option<OrderRow>> {
    const QUERY: &str = r#"
SELECT o.uid, o.owner, o.sell_token, o.buy_token, o.sell_token_account,
       o.buy_token_account, o.sell_amount, o.buy_amount, o.valid_to,
       o.kind::text AS kind, o.partially_fillable, o.app_data,
       o.creation_timestamp, o.order_pda,
       COALESCE(p.amount_withdrawn, 0) AS amount_withdrawn,
       COALESCE(p.amount_received, 0) AS amount_received,
       p.cancellation_timestamp
FROM solana.orders o
LEFT JOIN solana.order_pda p ON p.order_uid = o.uid
WHERE o.uid = $1
    "#;
    sqlx::query_as(QUERY)
        .bind(ByteArray(uid))
        .fetch_optional(ex)
        .await
        .context("read solana.orders by uid")
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::PgPool};

    async fn seed(pool: &PgPool, uid: [u8; 32], cancelled: bool) {
        sqlx::query("TRUNCATE solana.order_pda, solana.orders CASCADE")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda)
VALUES ($1, $2, $2, $3, $2, $2, 1000, 500, $4, 'sell'::OrderKind, false, $2, now(), $5)
            "#,
        )
        .bind(ByteArray(uid))
        .bind(ByteArray([0xAA; 32]))
        .bind(ByteArray([0xAB; 32]))
        .bind(i64::from(u32::MAX))
        .bind(ByteArray([0xB0; 32]))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO solana.order_pda (order_uid, created_by, amount_withdrawn, \
             cancellation_timestamp) VALUES ($1, $2, 400, CASE WHEN $3 THEN now() END)",
        )
        .bind(ByteArray(uid))
        .bind(ByteArray([0xAA; 32]))
        .bind(cancelled)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn reads_an_order_with_fill_state() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;

        let row = order_by_uid(&pool, uid).await.unwrap().unwrap();
        assert_eq!(row.uid, ByteArray(uid));
        assert_eq!(row.kind, "sell");
        assert_eq!(row.amount_withdrawn, BigDecimal::from(400));
        assert_eq!(row.amount_received, BigDecimal::from(0));
        assert!(row.cancellation_timestamp.is_none());

        assert!(order_by_uid(&pool, [0x99; 32]).await.unwrap().is_none());

        seed(&pool, uid, true).await;
        let row = order_by_uid(&pool, uid).await.unwrap().unwrap();
        assert!(row.cancellation_timestamp.is_some());
    }
}
