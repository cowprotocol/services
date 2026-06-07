use {
    crate::indexer::uniswap_v3::{LiquidityUpdateData, NewPoolData, PoolStateData, TickDeltaData},
    alloy_primitives::Address,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    num::ToPrimitive,
    number::conversions::u160_to_big_decimal,
    sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow},
};

fn bytes_to_addr(b: Vec<u8>) -> Result<Address> {
    Address::try_from(b.as_slice()).context("invalid address bytes")
}

fn address_bytes_list(addresses: &[Address]) -> Vec<&[u8]> {
    addresses.iter().map(|address| address.as_slice()).collect()
}

fn decode_pool_rows(rows: Vec<PgRow>) -> Result<Vec<PoolRow>> {
    rows.into_iter().map(PoolRow::try_from).collect()
}

pub async fn get_checkpoint(pool: &PgPool, contract: &Address) -> Result<Option<u64>> {
    let row = sqlx::query(
        "SELECT block_number FROM pool_indexer_checkpoints WHERE contract_address = $1",
    )
    .bind(contract.as_slice())
    .fetch_optional(pool)
    .await
    .context("get_checkpoint")?;

    Ok(row.map(|r| r.get::<i64, _>("block_number").cast_unsigned()))
}

pub async fn set_checkpoint(
    tx: &mut Transaction<'_, Postgres>,
    contract: &Address,
    block_number: u64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO pool_indexer_checkpoints (contract_address, block_number)
         VALUES ($1, $2)
         ON CONFLICT (contract_address) DO UPDATE SET block_number = EXCLUDED.block_number",
    )
    .bind(contract.as_slice())
    .bind(block_number.cast_signed())
    .execute(&mut **tx)
    .await
    .context("set_checkpoint")?;
    Ok(())
}

pub async fn insert_pools(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    pools: &[NewPoolData],
) -> Result<()> {
    if pools.is_empty() {
        return Ok(());
    }
    let len = pools.len();
    let mut addresses: Vec<&[u8]> = Vec::with_capacity(len);
    let mut token0s: Vec<&[u8]> = Vec::with_capacity(len);
    let mut token1s: Vec<&[u8]> = Vec::with_capacity(len);
    let mut fees: Vec<i32> = Vec::with_capacity(len);
    let mut t0_decimals: Vec<Option<i16>> = Vec::with_capacity(len);
    let mut t1_decimals: Vec<Option<i16>> = Vec::with_capacity(len);
    let mut t0_symbols: Vec<Option<String>> = Vec::with_capacity(len);
    let mut t1_symbols: Vec<Option<String>> = Vec::with_capacity(len);
    let mut created_blocks: Vec<i64> = Vec::with_capacity(len);
    for pool in pools {
        addresses.push(pool.address.as_slice());
        token0s.push(pool.token0.as_slice());
        token1s.push(pool.token1.as_slice());
        fees.push(pool.fee.cast_signed());
        t0_decimals.push(pool.token0_decimals.map(i16::from));
        t1_decimals.push(pool.token1_decimals.map(i16::from));
        t0_symbols.push(pool.token0_symbol.clone());
        t1_symbols.push(pool.token1_symbol.clone());
        created_blocks.push(pool.created_block.cast_signed());
    }

    sqlx::query(
        "INSERT INTO uniswap_v3_pools
             (address, factory, token0, token1, fee, token0_decimals, token1_decimals,
              token0_symbol, token1_symbol, created_block)
         SELECT t.addr, $1, t.t0, t.t1, t.fee, t.t0d, t.t1d, t.t0s, t.t1s, t.cblk
         FROM UNNEST($2::BYTEA[], $3::BYTEA[], $4::BYTEA[], $5::INT4[], $6::INT2[], $7::INT2[],
                     $8::TEXT[], $9::TEXT[], $10::INT8[])
              AS t(addr, t0, t1, fee, t0d, t1d, t0s, t1s, cblk)
         ON CONFLICT (address) DO NOTHING",
    )
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(token0s)
    .bind(token1s)
    .bind(fees)
    .bind(t0_decimals)
    .bind(t1_decimals)
    .bind(t0_symbols)
    .bind(t1_symbols)
    .bind(created_blocks)
    .execute(&mut **tx)
    .await
    .context("insert_pools")?;
    Ok(())
}

pub async fn upsert_pool_states(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    states: &[PoolStateData],
) -> Result<()> {
    if states.is_empty() {
        return Ok(());
    }
    let len = states.len();
    let mut addresses: Vec<&[u8]> = Vec::with_capacity(len);
    let mut block_numbers: Vec<i64> = Vec::with_capacity(len);
    let mut sqrt_prices: Vec<BigDecimal> = Vec::with_capacity(len);
    let mut liquidities: Vec<BigDecimal> = Vec::with_capacity(len);
    let mut ticks: Vec<i32> = Vec::with_capacity(len);
    for state in states {
        addresses.push(state.pool_address.as_slice());
        block_numbers.push(state.block_number.cast_signed());
        sqrt_prices.push(u160_to_big_decimal(&state.sqrt_price_x96));
        liquidities.push(BigDecimal::from(state.liquidity));
        ticks.push(state.tick);
    }

    sqlx::query(
        "WITH latest AS (
             SELECT DISTINCT ON (addr) addr, blk, sqrt, liq, tick
             FROM UNNEST($2::BYTEA[], $3::INT8[], $4::NUMERIC[], $5::NUMERIC[], $6::INT4[])
                  AS t(addr, blk, sqrt, liq, tick)
             ORDER BY addr, blk DESC
         )
         INSERT INTO uniswap_v3_pool_states
             (pool_address, block_number, sqrt_price_x96, liquidity, tick)
         SELECT l.addr, l.blk, l.sqrt, l.liq, l.tick
         FROM latest l
         WHERE EXISTS (
             SELECT 1 FROM uniswap_v3_pools
             WHERE address = l.addr AND factory = $1
         )
         ON CONFLICT (pool_address) DO UPDATE
             SET block_number   = EXCLUDED.block_number,
                 sqrt_price_x96 = EXCLUDED.sqrt_price_x96,
                 liquidity      = EXCLUDED.liquidity,
                 tick           = EXCLUDED.tick",
    )
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(block_numbers)
    .bind(sqrt_prices)
    .bind(liquidities)
    .bind(ticks)
    .execute(&mut **tx)
    .await
    .context("upsert_pool_states")?;
    Ok(())
}

pub async fn batch_update_pool_liquidity(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    updates: &[LiquidityUpdateData],
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = updates
        .iter()
        .map(|update| update.pool_address.as_slice())
        .collect();
    let liquidities: Vec<BigDecimal> = updates
        .iter()
        .map(|update| BigDecimal::from(update.liquidity))
        .collect();
    let block_numbers: Vec<i64> = updates
        .iter()
        .map(|update| update.block_number.cast_signed())
        .collect();

    sqlx::query(
        "WITH latest AS (
             SELECT DISTINCT ON (addr) addr, liq, blk
             FROM UNNEST($2::BYTEA[], $3::NUMERIC[], $4::INT8[]) AS t(addr, liq, blk)
             ORDER BY addr, blk DESC
         )
         UPDATE uniswap_v3_pool_states s
         SET liquidity = l.liq, block_number = l.blk
         FROM latest l
         WHERE s.pool_address = l.addr
           AND EXISTS (
               SELECT 1 FROM uniswap_v3_pools p
               WHERE p.address = l.addr AND p.factory = $1
           )",
    )
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(liquidities)
    .bind(block_numbers)
    .execute(&mut **tx)
    .await
    .context("batch_update_pool_liquidity")?;
    Ok(())
}

/// Returns the subset of `candidates` that are already persisted pools
/// belonging to `factory`. Used to gate event dispatch — only emitters
/// previously created by our factory should affect indexed state, otherwise
/// a malicious contract emitting the same event signatures could spoof
/// pool updates. New pools created in the current chunk's `PoolCreated`
/// events must be tracked separately by the caller; this lookup only
/// reflects committed DB state.
pub async fn known_pool_addresses(
    db: &PgPool,
    factory: &Address,
    candidates: &[Address],
) -> Result<std::collections::HashSet<Address>> {
    if candidates.is_empty() {
        return Ok(std::collections::HashSet::new());
    }
    let rows = sqlx::query(
        "SELECT address FROM uniswap_v3_pools WHERE factory = $1 AND address = ANY($2)",
    )
    .bind(factory.as_slice())
    .bind(address_bytes_list(candidates))
    .fetch_all(db)
    .await
    .context("known_pool_addresses")?;

    rows.into_iter()
        .map(|r| bytes_to_addr(r.get("address")))
        .collect()
}

/// Bulk-loads `(tick, liquidity)` for pools the caller is about to touch with
/// `Mint`/`Burn` events. Pools absent from `uniswap_v3_pool_states` (not yet
/// initialised) are omitted from the result — callers must treat absence as
/// "skip the liquidity update for this pool."
pub async fn get_base_pool_states(
    db: &PgPool,
    factory: &Address,
    addresses: &[Address],
) -> Result<std::collections::HashMap<Address, (i32, u128)>> {
    if addresses.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows = sqlx::query(
        "SELECT s.pool_address, s.tick, s.liquidity
         FROM uniswap_v3_pool_states s
         JOIN uniswap_v3_pools p ON p.address = s.pool_address
         WHERE p.factory = $1
           AND s.pool_address = ANY($2)",
    )
    .bind(factory.as_slice())
    .bind(address_bytes_list(addresses))
    .fetch_all(db)
    .await
    .context("get_base_pool_states")?;

    rows.into_iter()
        .map(|r| {
            let addr = bytes_to_addr(r.get("pool_address"))?;
            let tick: i32 = r.get("tick");
            let liquidity = r
                .get::<BigDecimal, _>("liquidity")
                .to_u128()
                .context("pool_states.liquidity value overflows u128")?;
            Ok((addr, (tick, liquidity)))
        })
        .collect()
}

pub async fn batch_update_ticks(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    deltas: &[TickDeltaData],
) -> Result<()> {
    if deltas.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = deltas
        .iter()
        .map(|delta| delta.pool_address.as_slice())
        .collect();
    let tick_idxs: Vec<i32> = deltas.iter().map(|delta| delta.tick_idx).collect();
    let delta_values: Vec<BigDecimal> = deltas
        .iter()
        .map(|delta| BigDecimal::from(delta.delta))
        .collect();

    // Two-pronged invariant for zero-net ticks (we never keep a row with
    // `liquidity_net = 0`):
    //
    //   1. INSERT path: skip rows whose aggregated delta is zero entirely, so no
    //      row with `liquidity_net = 0` is ever inserted. The `DELETE` at the
    //      bottom of the CTE can't see freshly-inserted rows in the same statement
    //      (Postgres modifying-CTE snapshot rules — sibling CTEs run against the
    //      pre-statement snapshot of the target table), so we have to gate this
    //      case at the INSERT side instead.
    //   2. UPDATE path: when an existing row's `+ EXCLUDED.liquidity_net` lands at
    //      zero, the row is in the pre-statement snapshot, so the `DELETE ... USING
    //      upserted` *can* see it and removes it.
    //
    // `into_chunk_changes` already filters `delta != 0` upstream, but that
    // filter only blocks single zero entries — two in-batch entries summing
    // to zero for the same `(pool, tick)` would still reach the SQL. The
    // `AND i.total_delta <> 0` clause closes that gap.
    sqlx::query(
        "WITH input AS (
             SELECT t.addr, t.tick_idx, SUM(t.delta) AS total_delta
             FROM UNNEST($2::BYTEA[], $3::INT4[], $4::NUMERIC[]) AS t(addr, tick_idx, delta)
             GROUP BY t.addr, t.tick_idx
         ),
         upserted AS (
             INSERT INTO uniswap_v3_ticks (pool_address, tick_idx, liquidity_net)
             SELECT i.addr, i.tick_idx, i.total_delta
             FROM input i
             WHERE i.total_delta <> 0
               AND EXISTS (
                   SELECT 1 FROM uniswap_v3_pools
                   WHERE address = i.addr AND factory = $1
               )
             ON CONFLICT (pool_address, tick_idx) DO UPDATE
                 SET liquidity_net = uniswap_v3_ticks.liquidity_net + EXCLUDED.liquidity_net
             RETURNING pool_address, tick_idx, liquidity_net
         )
         DELETE FROM uniswap_v3_ticks ticks
         USING upserted
         WHERE ticks.pool_address = upserted.pool_address
           AND ticks.tick_idx   = upserted.tick_idx
           AND upserted.liquidity_net = 0",
    )
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(tick_idxs)
    .bind(delta_values)
    .execute(&mut **tx)
    .await
    .context("batch_update_ticks")?;
    Ok(())
}

/// Insert/replace tick `liquidity_net` values directly (no delta accumulation).
/// Used by the subgraph seeder where the subgraph value IS the authoritative
/// net.
pub async fn batch_seed_ticks(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    ticks: &[TickDeltaData],
) -> Result<()> {
    if ticks.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = ticks
        .iter()
        .map(|tick| tick.pool_address.as_slice())
        .collect();
    let tick_idxs: Vec<i32> = ticks.iter().map(|tick| tick.tick_idx).collect();
    let values: Vec<BigDecimal> = ticks
        .iter()
        .map(|tick| BigDecimal::from(tick.delta))
        .collect();

    sqlx::query(
        "WITH input AS (
             SELECT t.addr, t.tick_idx, SUM(t.val) AS net
             FROM UNNEST($2::BYTEA[], $3::INT4[], $4::NUMERIC[]) AS t(addr, tick_idx, val)
             GROUP BY t.addr, t.tick_idx
         )
         INSERT INTO uniswap_v3_ticks (pool_address, tick_idx, liquidity_net)
         SELECT i.addr, i.tick_idx, i.net
         FROM input i
         WHERE EXISTS (
             SELECT 1 FROM uniswap_v3_pools
             WHERE address = i.addr AND factory = $1
         )
           AND i.net <> 0
         ON CONFLICT (pool_address, tick_idx) DO UPDATE
             SET liquidity_net = EXCLUDED.liquidity_net",
    )
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(tick_idxs)
    .bind(values)
    .execute(&mut **tx)
    .await
    .context("batch_seed_ticks")?;
    Ok(())
}

/// Deletes ticks for all pools owned by `factory`. Used by the subgraph
/// seeder to clear stale state before reseeding. Scoped to this factory so a
/// reseed on one factory doesn't wipe another's ticks.
pub async fn delete_ticks_for_factory(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM uniswap_v3_ticks t
         USING uniswap_v3_pools p
         WHERE p.address = t.pool_address
           AND p.factory = $1",
    )
    .bind(factory.as_slice())
    .execute(&mut **tx)
    .await
    .context("delete_ticks_for_factory")?;
    Ok(())
}

/// A pool with its current on-chain state (price, liquidity, tick).
pub struct PoolRow {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    pub fee: u32,
    pub token0_decimals: Option<u8>,
    pub token1_decimals: Option<u8>,
    pub token0_symbol: Option<String>,
    pub token1_symbol: Option<String>,
    pub sqrt_price_x96: BigDecimal,
    pub liquidity: BigDecimal,
    pub tick: i32,
}

impl TryFrom<PgRow> for PoolRow {
    type Error = anyhow::Error;

    fn try_from(r: PgRow) -> Result<Self> {
        Ok(Self {
            address: bytes_to_addr(r.get("address"))?,
            token0: bytes_to_addr(r.get("token0"))?,
            token1: bytes_to_addr(r.get("token1"))?,
            fee: r.get::<i32, _>("fee").cast_unsigned(),
            // The DB stores `-1` as the "tried, failed" sentinel written by
            // the decimals backfill task. Drop those back to `None` so callers
            // see "missing" rather than a misleading `Some(0)`.
            token0_decimals: r
                .get::<Option<i16>, _>("token0_decimals")
                .and_then(|d| u8::try_from(d).ok()),
            token1_decimals: r
                .get::<Option<i16>, _>("token1_decimals")
                .and_then(|d| u8::try_from(d).ok()),
            token0_symbol: r.get("token0_symbol"),
            token1_symbol: r.get("token1_symbol"),
            sqrt_price_x96: r.get("sqrt_price_x96"),
            liquidity: r.get("liquidity"),
            tick: r.get("tick"),
        })
    }
}

/// Fetches a page of pools ordered by address with their current state. Pass
/// `cursor = None` for the first page, or the previous page's last address for
/// keyset pagination.
pub async fn get_pools(pool: &PgPool, cursor: Option<Vec<u8>>, limit: u64) -> Result<Vec<PoolRow>> {
    let rows = sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s ON s.pool_address = p.address
         WHERE ($1::BYTEA IS NULL OR p.address > $1)
         ORDER BY p.address
         LIMIT $2",
    )
    .bind(cursor)
    .bind(limit.cast_signed())
    .fetch_all(pool)
    .await
    .context("get_pools")?;

    decode_pool_rows(rows)
}

pub struct TickRow {
    pub tick_idx: i32,
    pub liquidity_net: BigDecimal,
}

/// A tick tagged with its owning pool, used by bulk-tick queries that span
/// multiple pools.
pub struct PoolTickRow {
    pub pool_address: Address,
    pub tick_idx: i32,
    pub liquidity_net: BigDecimal,
}

/// Fetches pools matching any of `addresses` with their current state. Returns
/// fewer rows than requested when some addresses are unknown. Ordered by
/// address to give callers a stable iteration order.
pub async fn get_pools_by_ids(pool: &PgPool, addresses: &[Address]) -> Result<Vec<PoolRow>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s ON s.pool_address = p.address
         WHERE p.address = ANY($1)
         ORDER BY p.address",
    )
    .bind(address_bytes_list(addresses))
    .fetch_all(pool)
    .await
    .context("get_pools_by_ids")?;

    decode_pool_rows(rows)
}

/// Fetches ticks for multiple pools in one query. Rows are ordered by
/// `(pool_address, tick_idx)` so callers can group in a single pass.
///
/// No per-pool cap is applied: bulk callers already limit the number of pool
/// addresses they request (see the driver-side `POOL_IDS_PER_REQUEST`), and
/// real-world active-tick counts (~1500 on today's largest mainnet pool) are
/// far below any number where the response size becomes a concern. If that
/// changes, the right response is to surface truncation explicitly to the
/// caller (e.g. a `truncated: bool` per pool in the API response) rather than
/// silently drop tick rows — silently truncated tick data produces an
/// incorrect price curve that the driver can't detect.
pub async fn get_ticks_for_pools(pool: &PgPool, addresses: &[Address]) -> Result<Vec<PoolTickRow>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT pool_address, tick_idx, liquidity_net
         FROM uniswap_v3_ticks
         WHERE pool_address = ANY($1)
         ORDER BY pool_address, tick_idx",
    )
    .bind(address_bytes_list(addresses))
    .fetch_all(pool)
    .await
    .context("get_ticks_for_pools")?;

    rows.into_iter()
        .map(|r| {
            Ok(PoolTickRow {
                pool_address: bytes_to_addr(r.get("pool_address"))?,
                tick_idx: r.get("tick_idx"),
                liquidity_net: r.get("liquidity_net"),
            })
        })
        .collect()
}

pub async fn get_ticks(pool: &PgPool, pool_address: &Address) -> Result<Vec<TickRow>> {
    sqlx::query(
        "SELECT tick_idx, liquidity_net
         FROM uniswap_v3_ticks
         WHERE pool_address = $1
         ORDER BY tick_idx",
    )
    .bind(pool_address.as_slice())
    .fetch_all(pool)
    .await
    .context("get_ticks")
    .map(|rows| {
        rows.into_iter()
            .map(|r| TickRow {
                tick_idx: r.get("tick_idx"),
                liquidity_net: r.get("liquidity_net"),
            })
            .collect()
    })
}

/// Returns all distinct token addresses that have no symbol recorded yet.
pub async fn get_tokens_missing_symbols(pool: &PgPool) -> Result<Vec<Address>> {
    let rows = sqlx::query(
        "SELECT DISTINCT token FROM (
             SELECT token0 AS token FROM uniswap_v3_pools WHERE token0_symbol IS NULL
             UNION
             SELECT token1 AS token FROM uniswap_v3_pools WHERE token1_symbol IS NULL
         ) t",
    )
    .fetch_all(pool)
    .await
    .context("get_tokens_missing_symbols")?;

    rows.into_iter()
        .map(|r| bytes_to_addr(r.get("token")))
        .collect()
}

/// Returns all distinct token addresses that have no decimals recorded yet.
pub async fn get_tokens_missing_decimals(pool: &PgPool) -> Result<Vec<Address>> {
    let rows = sqlx::query(
        "SELECT DISTINCT token FROM (
             SELECT token0 AS token FROM uniswap_v3_pools WHERE token0_decimals IS NULL
             UNION
             SELECT token1 AS token FROM uniswap_v3_pools WHERE token1_decimals IS NULL
         ) t",
    )
    .fetch_all(pool)
    .await
    .context("get_tokens_missing_decimals")?;

    rows.into_iter()
        .map(|r| bytes_to_addr(r.get("token")))
        .collect()
}

/// Batched update of `token0_decimals` / `token1_decimals` for every pool
/// containing one of the provided tokens. Pass `-1` for entries that were
/// "tried, failed" so the next backfill pass's `IS NULL` filter skips them.
///
/// One round-trip via a writeable CTE: the side-by-side UPDATE ... FROM UNNEST
/// pattern would mis-handle pools where both `token0` and `token1` appear in
/// the batch (Postgres picks an arbitrary FROM row per target row, so only
/// one side would get set). Splitting into two separate UPDATEs keyed on each
/// side avoids that.
pub async fn batch_set_token_decimals(
    tx: &mut Transaction<'_, Postgres>,
    entries: &[(Address, i16)],
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let tokens: Vec<&[u8]> = entries.iter().map(|(t, _)| t.as_slice()).collect();
    let decimals: Vec<i16> = entries.iter().map(|(_, d)| *d).collect();

    sqlx::query(
        "WITH input AS (
             SELECT * FROM UNNEST($1::BYTEA[], $2::INT2[]) AS t(tok, dec)
         ),
         update_t0 AS (
             UPDATE uniswap_v3_pools p
             SET token0_decimals = i.dec
             FROM input i
             WHERE p.token0 = i.tok
               AND p.token0_decimals IS NULL
             RETURNING 1
         )
         UPDATE uniswap_v3_pools p
         SET token1_decimals = i.dec
         FROM input i
         WHERE p.token1 = i.tok
           AND p.token1_decimals IS NULL",
    )
    .bind(tokens)
    .bind(decimals)
    .execute(&mut **tx)
    .await
    .context("batch_set_token_decimals")?;

    Ok(())
}

/// Batched update of `token0_symbol` / `token1_symbol` for every pool
/// containing one of the provided tokens. Pass `""` for entries that were
/// "tried, failed" so the next backfill pass's `IS NULL` filter skips them.
/// See [`batch_set_token_decimals`] for the writeable-CTE rationale.
pub async fn batch_set_token_symbols(
    tx: &mut Transaction<'_, Postgres>,
    entries: &[(Address, String)],
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let tokens: Vec<&[u8]> = entries.iter().map(|(t, _)| t.as_slice()).collect();
    let symbols: Vec<&str> = entries.iter().map(|(_, s)| s.as_str()).collect();

    sqlx::query(
        "WITH input AS (
             SELECT * FROM UNNEST($1::BYTEA[], $2::TEXT[]) AS t(tok, sym)
         ),
         update_t0 AS (
             UPDATE uniswap_v3_pools p
             SET token0_symbol = i.sym
             FROM input i
             WHERE p.token0 = i.tok
               AND p.token0_symbol IS NULL
             RETURNING 1
         )
         UPDATE uniswap_v3_pools p
         SET token1_symbol = i.sym
         FROM input i
         WHERE p.token1 = i.tok
           AND p.token1_symbol IS NULL",
    )
    .bind(tokens)
    .bind(symbols)
    .execute(&mut **tx)
    .await
    .context("batch_set_token_symbols")?;

    Ok(())
}

pub async fn get_latest_indexed_block(pool: &PgPool) -> Result<Option<u64>> {
    let row = sqlx::query("SELECT MAX(block_number) AS max_block FROM pool_indexer_checkpoints")
        .fetch_one(pool)
        .await
        .context("get_latest_indexed_block")?;

    Ok(row
        .get::<Option<i64>, _>("max_block")
        .map(|b| b.cast_unsigned()))
}
