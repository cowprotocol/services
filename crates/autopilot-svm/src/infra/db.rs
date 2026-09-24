//! Database access for the Solana autopilot.

use {
    crate::domain::auction::{Order, OrderKind},
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, ToPrimitive},
    chain_types::solana::{AppData, IntentHash, Pubkey},
    database::byte_array::ByteArray,
    solana_sdk::clock::MAX_PROCESSING_AGE,
    sqlx::{PgExecutor, Postgres, QueryBuilder},
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
    pub created_on_chain: bool,
}

/// Orders open for solving: unexpired, settleable by a driver, not cancelled
/// and not fully filled. A pending sponsored order whose stored creation
/// transaction died at `block_height` is excluded, and a `None` height skips
/// that check rather than excluding everything. Settleable means the driver can
/// produce the order PDA: it already exists on chain (an order placed via
/// `CreateOrder` directly), or the driver can create it at settlement time from
/// a signed intent or a presigned transaction.
pub async fn open_orders(
    ex: impl PgExecutor<'_>,
    now_unix: i64,
    block_height: Option<i64>,
) -> Result<Vec<OrderRow>> {
    const QUERY: &str = r#"
SELECT o.uid, o.owner, o.sell_token, o.buy_token, o.sell_token_account,
       o.buy_token_account, o.sell_amount, o.buy_amount, o.valid_to,
       o.kind, o.partially_fillable, o.order_pda, o.app_data,
       p.order_uid IS NOT NULL AS created_on_chain
FROM solana.orders o
LEFT JOIN solana.order_pda p ON p.order_uid = o.uid
WHERE o.valid_to >= $1
  AND NOT COALESCE(p.is_reorged, false)
  AND (o.valid_from IS NULL OR o.valid_from <= $1)
  AND (o.intent_signature IS NOT NULL
       OR o.presigned_transaction IS NOT NULL
       OR p.order_uid IS NOT NULL)
  AND p.cancellation_timestamp IS NULL
  AND ($2::bigint IS NULL
       OR o.presigned_transaction IS NULL
       OR p.order_uid IS NOT NULL
       OR o.last_valid_block_height >= $2)
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
        .bind(block_height)
        .fetch_all(ex)
        .await
        .context("read open solana.orders")
}

/// Latest slot the indexer fully processed. `None` before the indexer's first
/// write. `solana.indexer_state` is a single-row table.
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

/// The stored creation transactions of the given orders that do not exist on
/// chain yet.
pub async fn pending_creations(
    ex: impl PgExecutor<'_>,
    uids: &[Vec<u8>],
) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
    const QUERY: &str = r#"
SELECT o.uid, o.presigned_transaction
FROM solana.orders o
LEFT JOIN solana.order_pda p ON p.order_uid = o.uid
WHERE o.uid = ANY($1)
  AND o.presigned_transaction IS NOT NULL
  AND p.order_uid IS NULL
    "#;
    sqlx::query_as(QUERY)
        .bind(uids)
        .fetch_all(ex)
        .await
        .context("read pending solana.orders creations")
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

/// Close a window whose settlement provably never reached the chain. The
/// window spans no slots, so it ends where it started. Only an open window
/// closes: an observed settlement outranks a driver's report.
pub async fn reject_settlement_window(
    ex: impl PgExecutor<'_>,
    auction_id: i64,
    solver: Pubkey,
    solution_uid: i64,
) -> Result<()> {
    const QUERY: &str = r#"
UPDATE solana.settlement_executions
SET outcome = 'rejected', end_timestamp = now(), end_slot = start_slot
WHERE auction_id = $1 AND solver = $2 AND solution_uid = $3 AND outcome IS NULL
    "#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(solver.0)
        .bind(solution_uid)
        .execute(ex)
        .await
        .context("reject settlement execution window")?;
    Ok(())
}

/// Close the auction's windows against the settlements the indexer recorded.
/// A settlement carries no solution uid, so a window is matched through its
/// solution's trade executions: the solver's settlement that traded one of
/// them is the one that executed it. A window already closed as timed out
/// upgrades to landed: the settlement executed, just late, and lateness stays
/// visible as `end_slot` past `deadline_slot`.
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
  AND EXISTS (
      SELECT 1
      FROM solana.proposed_trade_executions pte
      JOIN solana.trades t
        ON t.tx_signature = s.tx_signature
       AND t.instruction_index = s.instruction_index
       AND t.order_uid = pte.order_uid
      WHERE pte.auction_id = e.auction_id AND pte.solution_uid = e.solution_uid
  )
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

/// Orders inside a winning solution whose settlement transaction may still
/// land: a blockhash lifetime past the deadline slot has not run out, no
/// settlement of the auction traded the order yet, and the driver did not
/// reject the solution before sending it. A timed-out window keeps the hold,
/// its transaction may land until the blockhash expires. Landing is checked
/// per order through the settlement's trades: `settlements.solution_uid` is
/// unattributed, and one solver may win several solutions of one auction.
pub async fn in_flight_orders(
    ex: impl PgExecutor<'_>,
    tip_slot: i64,
) -> Result<Vec<ByteArray<32>>> {
    const QUERY: &str = r#"
SELECT DISTINCT pte.order_uid
FROM solana.competition_auctions ca
JOIN solana.proposed_solutions ps ON ps.auction_id = ca.id AND ps.is_winner
JOIN solana.proposed_trade_executions pte
  ON pte.auction_id = ca.id AND pte.solution_uid = ps.uid
WHERE ca.deadline_slot >= $1
  AND NOT EXISTS (
      SELECT 1
      FROM solana.settlements s
      JOIN solana.trades t
        ON t.tx_signature = s.tx_signature AND t.instruction_index = s.instruction_index
      WHERE s.auction_id = ca.id AND t.order_uid = pte.order_uid
  )
  AND NOT EXISTS (
      SELECT 1 FROM solana.settlement_executions se
      WHERE se.auction_id = ca.id AND se.solution_uid = ps.uid AND se.outcome = 'rejected'
  )
    "#;
    // The oldest deadline whose transaction can still land at the tip.
    let lifetime = i64::try_from(MAX_PROCESSING_AGE).expect("blockhash lifetime fits i64");
    let landable_deadline = tip_slot.saturating_sub(lifetime);
    sqlx::query_scalar(QUERY)
        .bind(landable_deadline)
        .fetch_all(ex)
        .await
        .context("read in-flight orders")
}

/// The solvable orders for a fresh auction cut.
pub async fn cut(
    ex: impl PgExecutor<'_>,
    now_unix: i64,
    block_height: Option<i64>,
) -> Result<Vec<Order>> {
    Ok(orders_from_rows(
        open_orders(ex, now_unix, block_height).await?,
    ))
}

/// Replace the current auction and answer the id its identity column
/// allocated, the source of sequential auction ids.
pub async fn replace_current_auction(
    pool: &sqlx::PgPool,
    tip_slot: i64,
    json: &serde_json::Value,
) -> Result<i64> {
    let mut tx = pool.begin().await.context("begin auction replacement")?;
    sqlx::query("DELETE FROM solana.auctions")
        .execute(&mut *tx)
        .await
        .context("delete the previous auction")?;
    let id = sqlx::query_scalar(
        "INSERT INTO solana.auctions (tip_slot, json) VALUES ($1, $2) RETURNING id",
    )
    .bind(tip_slot)
    .bind(sqlx::types::Json(json))
    .fetch_one(&mut *tx)
    .await
    .context("insert the current auction")?;
    tx.commit().await.context("commit auction replacement")?;
    Ok(id)
}

/// One execution inside a proposed solution.
pub struct ProposedTrade {
    pub order_uid: ByteArray<32>,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
}

/// One proposed solution of a competition, with its executions.
pub struct ProposedSolution {
    /// Autopilot-generated, unique within the auction.
    pub uid: i64,
    /// Solver-assigned, unique only within one driver response.
    pub id: i64,
    pub solver: ByteArray<32>,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub score: BigDecimal,
    pub trades: Vec<ProposedTrade>,
}

/// A winning solver's reference score: the winners' total score with that
/// solver's solutions removed.
pub struct ReferenceScore {
    pub solver: ByteArray<32>,
    pub score: BigDecimal,
}

/// A competition outcome as persisted after ranking.
pub struct Competition {
    pub auction_id: i64,
    pub tip_slot: i64,
    pub deadline_slot: i64,
    pub order_uids: Vec<Vec<u8>>,
    pub price_tokens: Vec<Vec<u8>>,
    pub price_values: Vec<BigDecimal>,
    pub solutions: Vec<ProposedSolution>,
    pub reference_scores: Vec<ReferenceScore>,
}

/// Persist a competition: the auction snapshot and every proposed solution
/// with its executions, in one transaction.
pub async fn persist_competition(pool: &sqlx::PgPool, competition: &Competition) -> Result<()> {
    let mut tx = pool.begin().await.context("begin competition persist")?;
    sqlx::query(
        "INSERT INTO solana.competition_auctions (id, tip_slot, deadline_slot, order_uids, \
         price_tokens, price_values) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(competition.auction_id)
    .bind(competition.tip_slot)
    .bind(competition.deadline_slot)
    .bind(&competition.order_uids)
    .bind(&competition.price_tokens)
    .bind(&competition.price_values)
    .execute(&mut *tx)
    .await
    .context("insert competition auction")?;
    if !competition.solutions.is_empty() {
        let mut insert = QueryBuilder::<Postgres>::new(
            "INSERT INTO solana.proposed_solutions (auction_id, uid, id, solver, is_winner, \
             filtered_out, score) ",
        );
        insert.push_values(&competition.solutions, |mut row, solution| {
            row.push_bind(competition.auction_id)
                .push_bind(solution.uid)
                .push_bind(solution.id)
                .push_bind(solution.solver)
                .push_bind(solution.is_winner)
                .push_bind(solution.filtered_out)
                .push_bind(&solution.score);
        });
        insert
            .build()
            .execute(&mut *tx)
            .await
            .context("insert proposed solutions")?;
    }
    let trades: Vec<_> = competition
        .solutions
        .iter()
        .flat_map(|solution| {
            solution
                .trades
                .iter()
                .map(move |trade| (solution.uid, trade))
        })
        .collect();
    if !trades.is_empty() {
        let mut insert = QueryBuilder::<Postgres>::new(
            "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, order_uid, \
             executed_sell, executed_buy) ",
        );
        insert.push_values(trades, |mut row, (solution_uid, trade)| {
            row.push_bind(competition.auction_id)
                .push_bind(solution_uid)
                .push_bind(trade.order_uid)
                .push_bind(&trade.executed_sell)
                .push_bind(&trade.executed_buy);
        });
        insert
            .build()
            .execute(&mut *tx)
            .await
            .context("insert proposed trade executions")?;
    }
    if !competition.reference_scores.is_empty() {
        let mut insert = QueryBuilder::<Postgres>::new(
            "INSERT INTO solana.reference_scores (auction_id, solver, reference_score) ",
        );
        insert.push_values(&competition.reference_scores, |mut row, reference| {
            row.push_bind(competition.auction_id)
                .push_bind(reference.solver)
                .push_bind(&reference.score);
        });
        insert
            .build()
            .execute(&mut *tx)
            .await
            .context("insert reference scores")?;
    }
    tx.commit().await.context("commit competition persist")
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
                    tracing::warn!(uid = %const_hex::encode_prefixed(uid.0), ?err, "skipping corrupt order row")
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
            created_on_chain: row.created_on_chain,
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
        super::{in_flight_orders, last_indexed_slot, open_orders},
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
            created_on_chain: true,
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
        // A pending sponsored order whose creation dies at height 150: kept
        // while the chain is below that height or the height is unknown,
        // dropped after.
        insert_order(&mut tx, 10, 2_000, true, database::solana::OrderKind::Sell).await;
        sqlx::query(
            r#"
UPDATE solana.orders
SET presigned_transaction = '\x01', last_valid_block_height = 150
WHERE uid = $1
            "#,
        )
        .bind(database::byte_array::ByteArray([10u8; 32]))
        .execute(&mut *tx)
        .await
        .unwrap();

        let uids = |orders: Vec<super::OrderRow>| -> Vec<u8> {
            orders.iter().map(|order| order.uid.0[0]).collect()
        };
        let orders = open_orders(&mut *tx, 1_000, Some(100)).await.unwrap();
        assert_eq!(uids(orders), vec![1, 5, 6, 10]);
        let orders = open_orders(&mut *tx, 1_000, None).await.unwrap();
        assert_eq!(uids(orders), vec![1, 5, 6, 10]);
        // Boundary: still alive when the chain height equals the stored height.
        let orders = open_orders(&mut *tx, 1_000, Some(150)).await.unwrap();
        assert_eq!(uids(orders), vec![1, 5, 6, 10]);
        let orders = open_orders(&mut *tx, 1_000, Some(151)).await.unwrap();
        assert_eq!(uids(orders), vec![1, 5, 6]);
    }

    /// Held: an order of a winning solution through its deadline slot plus
    /// the blockhash lifetime, a timed-out window included. Released: after
    /// that, once a settlement of the auction trades the order, or once the
    /// driver rejects its solution before sending. A solver's second winning
    /// solution keeps its hold when the first one lands. Orders of
    /// non-winning solutions are never held.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_in_flight_orders_follow_the_winning_settlement() {
        let pool = crate::test_db::pool().await;
        let mut tx = pool.begin().await.unwrap();
        for table in [
            "trades",
            "settlements",
            "settlement_executions",
            "proposed_trade_executions",
            "proposed_solutions",
            "competition_auctions",
        ] {
            sqlx::query(&format!("DELETE FROM solana.{table}"))
                .execute(&mut *tx)
                .await
                .unwrap();
        }
        let winner = ByteArray([0xEE; 32]);
        sqlx::query(
            "INSERT INTO solana.competition_auctions (id, tip_slot, deadline_slot, order_uids, \
             price_tokens, price_values) VALUES (77, 1, 100, '{}', '{}', '{}')",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        for (uid, solver, is_winner, order) in [
            (0i64, winner, true, 1u8),
            (1, ByteArray([0xEF; 32]), false, 2),
            (2, winner, true, 3),
        ] {
            sqlx::query(
                "INSERT INTO solana.proposed_solutions (auction_id, uid, id, solver, is_winner, \
                 filtered_out, score) VALUES (77, $1, 7, $2, $3, false, 1)",
            )
            .bind(uid)
            .bind(solver)
            .bind(is_winner)
            .execute(&mut *tx)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, \
                 order_uid, executed_sell, executed_buy) VALUES (77, $1, $2, 10, 20)",
            )
            .bind(uid)
            .bind(ByteArray([order; 32]))
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        async fn held(tx: &mut PgTransaction<'_>, tip: i64) -> Vec<u8> {
            let mut held: Vec<u8> = in_flight_orders(&mut **tx, tip)
                .await
                .unwrap()
                .iter()
                .map(|uid| uid.0[0])
                .collect();
            held.sort_unstable();
            held
        }

        assert_eq!(held(&mut tx, 100).await, vec![1, 3]);
        assert_eq!(held(&mut tx, 250).await, vec![1, 3]);
        assert_eq!(held(&mut tx, 251).await, Vec::<u8>::new());

        sqlx::query(
            "INSERT INTO solana.settlement_executions (auction_id, solver, solution_uid, \
             start_timestamp, start_slot, deadline_slot) VALUES (77, $1, 0, now(), 1, 100)",
        )
        .bind(winner)
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(held(&mut tx, 50).await, vec![1, 3]);
        sqlx::query(
            "UPDATE solana.settlement_executions SET outcome = 'rejected', end_slot = 2, \
             end_timestamp = now() WHERE auction_id = 77",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(held(&mut tx, 50).await, vec![3]);
        sqlx::query("UPDATE solana.settlement_executions SET outcome = NULL WHERE auction_id = 77")
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(held(&mut tx, 50).await, vec![1, 3]);
        sqlx::query(
            "UPDATE solana.settlement_executions SET outcome = 'timeout', end_slot = 100, \
             end_timestamp = now() WHERE auction_id = 77",
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(held(&mut tx, 150).await, vec![1, 3]);

        // The solver's settlement trades order 1 only: order 3, its other
        // winning solution, stays held until its own trade lands.
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, \
             auction_id) VALUES (10, $1, 0, $2, 77)",
        )
        .bind([9u8; 64])
        .bind(winner)
        .execute(&mut *tx)
        .await
        .unwrap();
        for order in [1u8, 3] {
            sqlx::query(
                "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid, \
                 sell_amount, buy_amount, fee_amount) VALUES ($1, 0, $2, 10, 20, 0)",
            )
            .bind([9u8; 64])
            .bind(ByteArray([order; 32]))
            .execute(&mut *tx)
            .await
            .unwrap();
            assert_eq!(
                held(&mut tx, 50).await,
                if order == 1 { vec![3] } else { vec![] }
            );
        }
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
