//! Database row types and queries.

use {
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    sqlx::PgConnection,
};

/// A DB trade that no settlement event resolves to, together with the token
/// and owner data of its order (NULL if the order is in neither the orders
/// nor the jit_orders table).
#[derive(sqlx::FromRow)]
pub struct DbTrade {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: Vec<u8>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub owner: Option<Vec<u8>>,
    pub sell_token: Option<Vec<u8>>,
    pub buy_token: Option<Vec<u8>>,
}

/// Same trade <-> settlement association as the backfill subcommand; does
/// not reference trades.tx_hash so it also runs against pre-V112 databases.
///
/// jit_orders.uid is not unique (its primary key is block_number/log_index and
/// one row is written per JIT fill), so it is joined through a LATERAL LIMIT 1
/// to keep exactly one row per trade; orders.uid is unique so a plain join is
/// fine there.
pub const ORPHANED_TRADES_QUERY: &str = r#"
SELECT
    t.block_number,
    t.log_index,
    t.order_uid,
    t.sell_amount,
    t.buy_amount,
    t.fee_amount,
    COALESCE(o.owner, j.owner) AS owner,
    COALESCE(o.sell_token, j.sell_token) AS sell_token,
    COALESCE(o.buy_token, j.buy_token) AS buy_token
FROM trades t
LEFT JOIN orders o ON o.uid = t.order_uid
LEFT JOIN LATERAL (
    SELECT owner, sell_token, buy_token
    FROM jit_orders j
    WHERE j.uid = t.order_uid
    LIMIT 1
) j ON true
WHERE NOT EXISTS (
    SELECT 1
    FROM settlements s
    WHERE s.block_number = t.block_number
    AND   s.log_index > t.log_index
)
ORDER BY t.block_number, t.log_index
"#;

/// A settlements row, looked up by tx hash.
#[derive(sqlx::FromRow)]
pub struct DbSettlement {
    pub block_number: i64,
    pub log_index: i64,
}

pub async fn db_settlements_by_tx(
    db: &mut PgConnection,
    tx_hash: &[u8],
) -> Result<Vec<DbSettlement>> {
    sqlx::query_as(
        "SELECT block_number, log_index FROM settlements WHERE tx_hash = $1 ORDER BY \
         block_number, log_index",
    )
    .bind(tx_hash)
    .fetch_all(db)
    .await
    .context("could not look up settlements by tx hash")
}

pub async fn db_trade_exists(db: &mut PgConnection, block: u64, log_index: u64) -> Result<bool> {
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM trades WHERE block_number = $1 AND log_index = $2)",
    )
    .bind(block.cast_signed())
    .bind(log_index.cast_signed())
    .fetch_one(db)
    .await
    .context("could not look up trade")?;
    Ok(exists)
}

/// A settlements row of a block range, for the forward `verify` scan.
#[derive(sqlx::FromRow)]
pub struct DbSettlementInRange {
    pub block_number: i64,
    pub log_index: i64,
    pub solver: Vec<u8>,
    pub tx_hash: Vec<u8>,
}

pub async fn db_settlements_in_range(
    db: &mut PgConnection,
    from: u64,
    to: u64,
) -> Result<Vec<DbSettlementInRange>> {
    sqlx::query_as(
        "SELECT block_number, log_index, solver, tx_hash FROM settlements WHERE block_number \
         BETWEEN $1 AND $2 ORDER BY block_number, log_index",
    )
    .bind(from.cast_signed())
    .bind(to.cast_signed())
    .fetch_all(db)
    .await
    .context("could not query settlements in range")
}

/// A trades row of a block range, for the forward `verify` scan. `tx_hash` is
/// NULL for pre-V112 rows, and also when the column does not exist yet (a
/// `NULL::bytea` literal is selected then; see `db_trades_in_range`).
#[derive(sqlx::FromRow)]
pub struct DbTradeInRange {
    pub block_number: i64,
    pub log_index: i64,
    pub order_uid: Vec<u8>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub fee_amount: BigDecimal,
    pub tx_hash: Option<Vec<u8>>,
}

pub async fn db_trades_in_range(
    db: &mut PgConnection,
    from: u64,
    to: u64,
    have_tx_hash: bool,
) -> Result<Vec<DbTradeInRange>> {
    let tx_hash = if have_tx_hash {
        "tx_hash"
    } else {
        "NULL::bytea AS tx_hash"
    };
    let query = format!(
        "SELECT block_number, log_index, order_uid, sell_amount, buy_amount, fee_amount, \
         {tx_hash} FROM trades WHERE block_number BETWEEN $1 AND $2 ORDER BY block_number, \
         log_index"
    );
    sqlx::query_as(&query)
        .bind(from.cast_signed())
        .bind(to.cast_signed())
        .fetch_all(db)
        .await
        .context("could not query trades in range")
}

/// Whether the trades.tx_hash column (migration V112) exists yet.
pub async fn trades_have_tx_hash(db: &mut PgConnection) -> Result<bool> {
    let (exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema = \
         current_schema() AND table_name = 'trades' AND column_name = 'tx_hash')",
    )
    .fetch_one(db)
    .await
    .context("could not check for trades.tx_hash")?;
    Ok(exists)
}

/// Row counts and indexed block ranges of the event tables; logged as a sanity
/// check so an empty or wrong database is obvious before scanning for orphans.
#[derive(sqlx::FromRow)]
pub struct TableStats {
    pub trades: i64,
    pub settlements: i64,
    pub min_trade_block: Option<i64>,
    pub max_trade_block: Option<i64>,
    pub min_settlement_block: Option<i64>,
    pub max_settlement_block: Option<i64>,
}

pub async fn table_stats(db: &mut PgConnection) -> Result<TableStats> {
    sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM trades) AS trades,
            (SELECT count(*) FROM settlements) AS settlements,
            (SELECT min(block_number) FROM trades) AS min_trade_block,
            (SELECT max(block_number) FROM trades) AS max_trade_block,
            (SELECT min(block_number) FROM settlements) AS min_settlement_block,
            (SELECT max(block_number) FROM settlements) AS max_settlement_block",
    )
    .fetch_one(db)
    .await
    .context("could not query table stats")
}
