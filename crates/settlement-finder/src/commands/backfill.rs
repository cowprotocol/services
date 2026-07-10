//! The `backfill` subcommand: fill trades.tx_hash (introduced in V112) for
//! rows indexed before the column existed.
//!
//! Port of the former database/backfills/backfill_trades_tx_hash.sql: a trade
//! belongs to the settlement whose Settlement event is the first one (lowest
//! log index) after the trade's Trade event in the same block, so each row is
//! resolved by looking up exactly that settlements row. Trades without a
//! matching settlements row (e.g. gaps in event indexing) are left NULL; run
//! `stats` beforehand to gauge how many such rows to expect and `check` to
//! locate them on-chain.
//!
//! The update runs in batches over the primary key, committing after each one,
//! so it never holds long locks and can be aborted and re-run at any time
//! (already backfilled rows are skipped through the `tx_hash IS NULL` filter;
//! rows written by the event indexer since V112 already contain the hash).

use {
    crate::db::trades_have_tx_hash,
    anyhow::{Context, Result, ensure},
    sqlx::{Connection, PgConnection},
};

const RESOLVE_TX_HASH: &str = "(
    SELECT s.tx_hash
    FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
    ORDER BY s.log_index
    LIMIT 1
)";

/// Restricts the UPDATE to rows that actually resolve, so unresolvable rows
/// are not rewritten (NULL to NULL) on every run and rows_affected reports the
/// real progress.
const RESOLVABLE: &str = "EXISTS (
    SELECT 1
    FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
)";

/// The last primary key of the batch starting after the cursor, i.e. the
/// inclusive upper bound of the batch (NULL once fewer than a full batch is
/// left).
const NEXT_BATCH_END: &str = "
SELECT t.block_number, t.log_index
FROM trades t
WHERE (t.block_number, t.log_index) > ($1, $2)
ORDER BY t.block_number, t.log_index
OFFSET $3
LIMIT 1
";

pub async fn backfill_cmd(db_url: &str, batch_size: i64, apply: bool) -> Result<()> {
    ensure!(batch_size > 0, "--batch-size must be positive");
    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;
    ensure!(
        trades_have_tx_hash(&mut db).await?,
        "trades.tx_hash does not exist; apply migration V112 first"
    );

    if !apply {
        tracing::info!(
            "counting backfillable rows (scans all NULL tx_hash rows, may take a while)"
        );
        let (total, resolvable): (i64, i64) = sqlx::query_as(
            "SELECT count(*), count(*) FILTER (WHERE EXISTS (
                SELECT 1 FROM settlements s
                WHERE s.block_number = t.block_number
                AND   s.log_index > t.log_index
             )) FROM trades t WHERE t.tx_hash IS NULL",
        )
        .fetch_one(&mut db)
        .await?;
        tracing::info!(
            total_null = total,
            resolvable,
            unresolvable = total - resolvable,
            "dry run complete; re-run with --apply to backfill (unresolvable rows stay NULL, \
             investigate them with check)"
        );
        return Ok(());
    }

    // Abort (and re-run later) instead of queueing behind long-lived locks.
    sqlx::query("SET lock_timeout = '10s'")
        .execute(&mut db)
        .await?;

    // Exclusive lower bound of the current batch.
    let mut cursor = (-1i64, -1i64);
    let mut total = 0u64;
    loop {
        let batch_end: Option<(i64, i64)> = sqlx::query_as(NEXT_BATCH_END)
            .bind(cursor.0)
            .bind(cursor.1)
            .bind(batch_size - 1)
            .fetch_optional(&mut db)
            .await?;

        // Each UPDATE is its own transaction, committing per batch.
        let updated = match batch_end {
            Some((block, log_index)) => {
                sqlx::query(&format!(
                    "UPDATE trades t SET tx_hash = {RESOLVE_TX_HASH}
                     WHERE (t.block_number, t.log_index) > ($1, $2)
                     AND   (t.block_number, t.log_index) <= ($3, $4)
                     AND   t.tx_hash IS NULL
                     AND   {RESOLVABLE}"
                ))
                .bind(cursor.0)
                .bind(cursor.1)
                .bind(block)
                .bind(log_index)
                .execute(&mut db)
                .await?
            }
            // Fewer than batch_size rows left; process them without an upper
            // bound.
            None => {
                sqlx::query(&format!(
                    "UPDATE trades t SET tx_hash = {RESOLVE_TX_HASH}
                     WHERE (t.block_number, t.log_index) > ($1, $2)
                     AND   t.tx_hash IS NULL
                     AND   {RESOLVABLE}"
                ))
                .bind(cursor.0)
                .bind(cursor.1)
                .execute(&mut db)
                .await?
            }
        };
        total += updated.rows_affected();

        match batch_end {
            Some(end) => {
                tracing::info!(backfilled = total, at_block = end.0, "backfill progress");
                cursor = end;
            }
            None => break,
        }
    }

    let (remaining,): (i64,) = sqlx::query_as("SELECT count(*) FROM trades WHERE tx_hash IS NULL")
        .fetch_one(&mut db)
        .await?;
    tracing::info!(
        backfilled = total,
        remaining_null = remaining,
        "backfill complete; consider running VACUUM ANALYZE trades; investigate remaining NULL \
         rows with check"
    );
    Ok(())
}
