//! Database access for the Solana autopilot.

use {
    crate::domain::auction::{Auction, Order, OrderKind},
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, ToPrimitive},
    chain_types::solana::{AppData, IntentHash, Pubkey},
    database::byte_array::ByteArray,
    sqlx::PgExecutor,
};

/// The channel the schema's `solana.settlements` trigger notifies.
pub const SETTLEMENT_FINALIZED_CHANNEL: &str = "solana_settlement_finalized";

/// The `solana.orders` columns auction assembly reads.
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
    pub kind: database::solana::OrderKind,
    pub partially_fillable: bool,
    pub order_pda: ByteArray<32>,
    pub app_data: ByteArray<32>,
}

/// Orders open for solving: unexpired, settleable by a driver, not cancelled
/// and not fully filled. Settleable means the driver can produce the order
/// PDA: it already exists on chain (an order placed via `CreateOrder`
/// directly), or the driver can create it at settlement time from a signed
/// intent or a presigned transaction.
pub async fn open_orders(ex: impl PgExecutor<'_>, now_unix: i64) -> Result<Vec<OrderRow>> {
    const QUERY: &str = r#"
SELECT o.uid, o.owner, o.sell_token, o.buy_token, o.sell_token_account,
       o.buy_token_account, o.sell_amount, o.buy_amount, o.valid_to,
       o.kind, o.partially_fillable, o.order_pda, o.app_data
FROM solana.orders o
LEFT JOIN solana.order_pda p ON p.order_uid = o.uid
WHERE o.valid_to >= $1
  AND (o.valid_from IS NULL OR o.valid_from <= $1)
  AND (o.intent_signature IS NOT NULL
       OR o.presigned_transaction IS NOT NULL
       OR p.order_uid IS NOT NULL)
  AND p.cancellation_timestamp IS NULL
  AND COALESCE(
      CASE o.kind
          WHEN 'sell' THEN p.amount_withdrawn < o.sell_amount
          ELSE p.amount_received < o.buy_amount
      END,
      true)
ORDER BY o.uid
    "#;
    sqlx::query_as(QUERY)
        .bind(now_unix)
        .fetch_all(ex)
        .await
        .context("read open solana.orders")
}

/// Latest slot the indexer fully processed. `None` before the indexer's first
/// write. `solana.indexer_state` is a single-row table.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by the freshness gating")
)]
pub async fn last_indexed_slot(ex: impl PgExecutor<'_>) -> Result<Option<i64>> {
    const QUERY: &str = r#"SELECT slot FROM solana.indexer_state"#;
    sqlx::query_scalar(QUERY)
        .fetch_optional(ex)
        .await
        .context("read solana.indexer_state slot")
}

/// A window closed as landed, for the caller's log line.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct LandedWindow {
    pub solver: ByteArray<32>,
    pub end_slot: i64,
    pub submitted_signature: ByteArray<64>,
}

/// Open a settlement-execution window for a dispatched settlement.
pub async fn open_settlement_window(
    ex: impl PgExecutor<'_>,
    auction_id: i64,
    solver: Pubkey,
    solution_uid: i64,
    start_slot: i64,
    deadline_slot: i64,
) -> Result<()> {
    const QUERY: &str = r#"
INSERT INTO solana.settlement_executions
    (auction_id, solver, solution_uid, start_timestamp, start_slot, deadline_slot)
VALUES ($1, $2, $3, now(), $4, $5)
ON CONFLICT (auction_id, solver, solution_uid) DO NOTHING
    "#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(solver.0)
        .bind(solution_uid)
        .bind(start_slot)
        .bind(deadline_slot)
        .execute(ex)
        .await
        .context("open settlement execution window")?;
    Ok(())
}

/// Close the auction's windows against the settlements the indexer recorded,
/// matching each window to its solver's settlement. A window already closed
/// as timed out upgrades to landed: the settlement executed, just late, and
/// lateness stays visible as `end_slot` past `deadline_slot`.
///
/// A settlement carries no solution uid, so a solver holding several windows
/// of one auction closes all of them on its first settlement. Correct while
/// one solver wins at most one solution per auction.
pub async fn close_landed_windows(
    ex: impl PgExecutor<'_>,
    auction_id: i64,
) -> Result<Vec<LandedWindow>> {
    const QUERY: &str = r#"
UPDATE solana.settlement_executions e
SET outcome = 'landed', end_timestamp = now(), end_slot = s.slot,
    submitted_signature = s.tx_signature
FROM solana.settlements s
WHERE e.auction_id = $1
  AND s.auction_id = e.auction_id
  AND s.solver = e.solver
  AND (e.outcome IS NULL OR e.outcome = 'timeout')
RETURNING e.solver, e.end_slot, e.submitted_signature
    "#;
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_all(ex)
        .await
        .context("close landed settlement execution windows")
}

/// Close every open window whose deadline is at or before the slot as timed
/// out, returning their auction ids.
pub async fn expire_settlement_windows(ex: impl PgExecutor<'_>, slot: i64) -> Result<Vec<i64>> {
    const QUERY: &str = r#"
UPDATE solana.settlement_executions
SET outcome = 'timeout', end_timestamp = now(), end_slot = $1
WHERE outcome IS NULL AND deadline_slot <= $1
RETURNING auction_id
    "#;
    sqlx::query_scalar(QUERY)
        .bind(slot)
        .fetch_all(ex)
        .await
        .context("expire settlement execution windows")
}

/// Auction ids of open windows, the seed re-read set.
pub async fn open_window_auction_ids(ex: impl PgExecutor<'_>) -> Result<Vec<i64>> {
    sqlx::query_scalar("SELECT auction_id FROM solana.settlement_executions WHERE outcome IS NULL")
        .fetch_all(ex)
        .await
        .context("read open settlement execution windows")
}

/// Cut an auction from the open orders.
pub async fn cut(ex: impl PgExecutor<'_>, id: i64, now_unix: i64) -> Result<Auction> {
    let orders = orders_from_rows(open_orders(ex, now_unix).await?);
    Ok(Auction { id, orders })
}

/// A row the indexer wrote always converts (on-chain values fit the domain
/// types), so a failure means corrupt data. The corrupt order is skipped
/// instead of failing the cut, which would block solving for every other
/// order.
fn orders_from_rows(rows: Vec<OrderRow>) -> Vec<Order> {
    rows.into_iter()
        .filter_map(|row| {
            let uid = row.uid;
            Order::try_from(row)
                .map_err(|err| {
                    tracing::warn!(uid = %const_hex::encode(uid.0), ?err, "skipping corrupt order row")
                })
                .ok()
        })
        .collect()
}

impl TryFrom<OrderRow> for Order {
    type Error = anyhow::Error;

    fn try_from(row: OrderRow) -> Result<Self> {
        Ok(Order {
            uid: IntentHash(row.uid.0),
            owner: Pubkey(row.owner.0),
            sell_token: Pubkey(row.sell_token.0),
            buy_token: Pubkey(row.buy_token.0),
            sell_token_account: Pubkey(row.sell_token_account.0),
            buy_token_account: Pubkey(row.buy_token_account.0),
            sell_amount: to_amount(&row.sell_amount).context("sell_amount")?,
            buy_amount: to_amount(&row.buy_amount).context("buy_amount")?,
            valid_to: row.valid_to.try_into().context("valid_to")?,
            kind: match row.kind {
                database::solana::OrderKind::Sell => OrderKind::Sell,
                database::solana::OrderKind::Buy => OrderKind::Buy,
            },
            partially_fillable: row.partially_fillable,
            order_pda: Pubkey(row.order_pda.0),
            app_data: AppData(row.app_data.0),
        })
    }
}

/// Token amounts are `numeric(20,0)` in the database, u64 on chain.
fn to_amount(value: &BigDecimal) -> Result<u64> {
    value
        .to_u64()
        .with_context(|| format!("amount {value} does not fit u64"))
}

#[cfg(test)]
mod tests {
    use {
        super::{last_indexed_slot, open_orders},
        bigdecimal::BigDecimal,
        database::byte_array::ByteArray,
        sqlx::PgTransaction,
    };

    fn conversion_row() -> super::OrderRow {
        super::OrderRow {
            uid: ByteArray([1; 32]),
            owner: ByteArray([2; 32]),
            sell_token: ByteArray([3; 32]),
            buy_token: ByteArray([4; 32]),
            sell_token_account: ByteArray([5; 32]),
            buy_token_account: ByteArray([6; 32]),
            sell_amount: BigDecimal::from(u64::MAX),
            buy_amount: BigDecimal::from(1_000u64),
            valid_to: 42,
            kind: database::solana::OrderKind::Sell,
            partially_fillable: false,
            order_pda: ByteArray([7; 32]),
            app_data: ByteArray([0; 32]),
        }
    }

    #[test]
    fn converts_a_row_and_rejects_out_of_range_values() {
        let order = super::Order::try_from(conversion_row()).unwrap();
        assert_eq!(order.sell_amount, u64::MAX);
        assert_eq!(order.kind, crate::domain::auction::OrderKind::Sell);

        let mut too_big = conversion_row();
        too_big.sell_amount = BigDecimal::from(u64::MAX) + BigDecimal::from(1u64);
        assert!(super::Order::try_from(too_big).is_err());
    }

    #[test]
    fn a_corrupt_row_is_skipped_not_fatal() {
        let mut corrupt = conversion_row();
        corrupt.sell_amount = BigDecimal::from(u64::MAX) + BigDecimal::from(1u64);
        let orders = super::orders_from_rows(vec![conversion_row(), corrupt]);
        assert_eq!(orders.len(), 1);
    }

    async fn insert_order(
        tx: &mut PgTransaction<'_>,
        n: u8,
        valid_to: i64,
        signed: bool,
        kind: database::solana::OrderKind,
    ) {
        sqlx::query(
            r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, intent_signature, creation_timestamp, order_pda)
VALUES ($1, $2, $2, $2, $2, $2, 1000, 2000, $3, $6, false, $2, $4, now(), $5)
            "#,
        )
        .bind(ByteArray([n; 32]))
        .bind(ByteArray([0xAA; 32]))
        .bind(valid_to)
        .bind(signed.then_some(ByteArray([0xBB; 64])))
        .bind(ByteArray([n | 0x80; 32]))
        .bind(kind)
        .execute(&mut **tx)
        .await
        .unwrap();
    }

    async fn insert_pda(
        tx: &mut PgTransaction<'_>,
        n: u8,
        cancelled: bool,
        withdrawn: i64,
        received: i64,
    ) {
        sqlx::query(
            r#"
INSERT INTO solana.order_pda (order_uid, created_by, cancellation_timestamp,
    amount_withdrawn, amount_received)
VALUES ($1, $2, CASE WHEN $3 THEN now() END, $4, $5)
            "#,
        )
        .bind(ByteArray([n; 32]))
        .bind(ByteArray([0xCC; 32]))
        .bind(cancelled)
        .bind(withdrawn)
        .bind(received)
        .execute(&mut **tx)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_open_orders_applies_the_solvability_predicates() {
        let pool = crate::test_db::pool().await;
        let mut tx = pool.begin().await.unwrap();

        for table in ["trades", "order_pda", "orders"] {
            sqlx::query(&format!("DELETE FROM solana.{table}"))
                .execute(&mut *tx)
                .await
                .unwrap();
        }

        // Kept: signed and unexpired, no PDA yet.
        insert_order(&mut tx, 1, 2_000, true, database::solana::OrderKind::Sell).await;
        // Dropped: expired.
        insert_order(&mut tx, 2, 500, true, database::solana::OrderKind::Sell).await;
        // Dropped: no PDA and nothing for the driver to create it from.
        insert_order(&mut tx, 3, 2_000, false, database::solana::OrderKind::Sell).await;
        // Dropped: cancelled on chain.
        insert_order(&mut tx, 4, 2_000, true, database::solana::OrderKind::Sell).await;
        insert_pda(&mut tx, 4, true, 0, 0).await;
        // Dropped: not yet valid.
        insert_order(&mut tx, 9, 2_000, true, database::solana::OrderKind::Sell).await;
        sqlx::query(r#"UPDATE solana.orders SET valid_from = 1_500 WHERE uid = $1"#)
            .bind(database::byte_array::ByteArray([9u8; 32]))
            .execute(&mut *tx)
            .await
            .unwrap();
        // Kept: live PDA, partially filled.
        insert_order(&mut tx, 5, 2_000, true, database::solana::OrderKind::Sell).await;
        insert_pda(&mut tx, 5, false, 999, 0).await;
        // Kept: created directly on chain, no off-chain material.
        insert_order(&mut tx, 6, 2_000, false, database::solana::OrderKind::Sell).await;
        insert_pda(&mut tx, 6, false, 0, 0).await;
        // Dropped: sell side fully withdrawn.
        insert_order(&mut tx, 7, 2_000, true, database::solana::OrderKind::Sell).await;
        insert_pda(&mut tx, 7, false, 1_000, 0).await;
        // Dropped: buy side fully received.
        insert_order(&mut tx, 8, 2_000, true, database::solana::OrderKind::Buy).await;
        insert_pda(&mut tx, 8, false, 0, 2_000).await;

        let orders = open_orders(&mut *tx, 1_000).await.unwrap();
        let uids: Vec<u8> = orders.iter().map(|order| order.uid.0[0]).collect();
        assert_eq!(uids, vec![1, 5, 6]);
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_last_indexed_slot_roundtrip() {
        let pool = crate::test_db::pool().await;
        let mut tx = pool.begin().await.unwrap();

        sqlx::query(r#"DELETE FROM solana.indexer_state"#)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(last_indexed_slot(&mut *tx).await.unwrap(), None);

        sqlx::query(r#"INSERT INTO solana.indexer_state (slot, finalized_slot) VALUES (42, 0)"#)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(last_indexed_slot(&mut *tx).await.unwrap(), Some(42));
    }
}
