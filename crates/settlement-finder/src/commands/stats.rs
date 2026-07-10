//! The `stats` subcommand: report how consistent the trades <-> settlements
//! association is.
//!
//! Port of the former database/backfills/backfill_trades_tx_hash_check.sql,
//! with one deviation: trades_without_settlement uses the log_index-aware
//! association (settlement event after the trade in the same block), matching
//! the backfill and check subcommands, instead of the block-only variant.

use {
    anyhow::{Context, Result},
    serde_json::json,
    sqlx::{Connection, PgConnection},
};

/// * trades_without_settlement: trades that no settlement event resolves to,
///   i.e. the rows `backfill` would leave NULL (e.g. gaps in event indexing or
///   failed reorg protection; locate them with `check`).
/// * settlements_without_trades: settlements that no trade resolves to. Not
///   necessarily an indexing gap: settle() calls with an empty trades array
///   legitimately emit no Trade events.
///
/// The block ranges show where the unmatched rows cluster. Scans both tables
/// once with an index probe per row, so expect it to take a few minutes on the
/// bigger databases.
const STATS_QUERY: &str = r#"
WITH unmatched_trades AS (
    SELECT t.block_number
    FROM trades t
    WHERE NOT EXISTS (
        SELECT 1
        FROM settlements s
        WHERE s.block_number = t.block_number
        AND   s.log_index > t.log_index
    )
),
unmatched_settlements AS (
    SELECT s.block_number
    FROM settlements s
    WHERE NOT EXISTS (
        SELECT 1
        FROM trades t
        WHERE t.block_number = s.block_number
        AND   t.log_index < s.log_index
        -- only trades after the previous settlement in the same block
        -- resolve to s
        AND   t.log_index > COALESCE((
            SELECT max(prev.log_index)
            FROM settlements prev
            WHERE prev.block_number = s.block_number
            AND   prev.log_index < s.log_index
        ), -1)
    )
)
SELECT
    (SELECT count(*) FROM unmatched_trades) AS trades_without_settlement,
    (SELECT min(block_number) FROM unmatched_trades) AS first_unmatched_trade_block,
    (SELECT max(block_number) FROM unmatched_trades) AS last_unmatched_trade_block,
    (SELECT count(*) FROM unmatched_settlements) AS settlements_without_trades,
    (SELECT min(block_number) FROM unmatched_settlements) AS first_unmatched_settlement_block,
    (SELECT max(block_number) FROM unmatched_settlements) AS last_unmatched_settlement_block
"#;

#[derive(sqlx::FromRow)]
struct Stats {
    trades_without_settlement: i64,
    first_unmatched_trade_block: Option<i64>,
    last_unmatched_trade_block: Option<i64>,
    settlements_without_trades: i64,
    first_unmatched_settlement_block: Option<i64>,
    last_unmatched_settlement_block: Option<i64>,
}

pub async fn stats_cmd(db_url: &str, json: bool) -> Result<()> {
    let mut db = PgConnection::connect(db_url)
        .await
        .context("could not connect to database")?;
    let table_stats = crate::db::table_stats(&mut db).await?;
    tracing::info!(
        trades = table_stats.trades,
        settlements = table_stats.settlements,
        "scanning association consistency (a few minutes on big databases)"
    );
    let stats: Stats = sqlx::query_as(STATS_QUERY)
        .fetch_one(&mut db)
        .await
        .context("could not query association stats")?;

    if json {
        let doc = json!({
            "trades": table_stats.trades,
            "settlements": table_stats.settlements,
            "min_trade_block": table_stats.min_trade_block,
            "max_trade_block": table_stats.max_trade_block,
            "min_settlement_block": table_stats.min_settlement_block,
            "max_settlement_block": table_stats.max_settlement_block,
            "trades_without_settlement": stats.trades_without_settlement,
            "first_unmatched_trade_block": stats.first_unmatched_trade_block,
            "last_unmatched_trade_block": stats.last_unmatched_trade_block,
            "settlements_without_trades": stats.settlements_without_trades,
            "first_unmatched_settlement_block": stats.first_unmatched_settlement_block,
            "last_unmatched_settlement_block": stats.last_unmatched_settlement_block,
        });
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        let range = |first: Option<i64>, last: Option<i64>| match (first, last) {
            (Some(first), Some(last)) => format!("{first} .. {last}"),
            _ => "-".to_string(),
        };
        println!("{:<28}  {:>12}  blocks", "metric", "count");
        println!(
            "{:<28}  {:>12}  {}",
            "-".repeat(28),
            "-".repeat(12),
            "-".repeat(16)
        );
        println!(
            "{:<28}  {:>12}  {}",
            "trades",
            table_stats.trades,
            range(table_stats.min_trade_block, table_stats.max_trade_block)
        );
        println!(
            "{:<28}  {:>12}  {}",
            "settlements",
            table_stats.settlements,
            range(
                table_stats.min_settlement_block,
                table_stats.max_settlement_block
            )
        );
        println!(
            "{:<28}  {:>12}  {}",
            "trades_without_settlement",
            stats.trades_without_settlement,
            range(
                stats.first_unmatched_trade_block,
                stats.last_unmatched_trade_block
            )
        );
        println!(
            "{:<28}  {:>12}  {}",
            "settlements_without_trades",
            stats.settlements_without_trades,
            range(
                stats.first_unmatched_settlement_block,
                stats.last_unmatched_settlement_block
            )
        );
    }
    if stats.settlements_without_trades > 0 {
        tracing::info!(
            "settlements without trades are not necessarily indexing gaps: settle() calls with an \
             empty trades array legitimately emit no Trade events"
        );
    }
    Ok(())
}
