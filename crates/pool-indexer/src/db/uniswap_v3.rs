use {
    crate::indexer::uniswap_v3::{LiquidityUpdateData, NewPoolData, PoolStateData, TickDeltaData},
    alloy_primitives::Address,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    num::BigInt,
    number::conversions::u160_to_big_decimal,
    sqlx::{PgPool, Postgres, Row, Transaction, postgres::PgRow},
};

fn bytes_to_addr(b: Vec<u8>) -> Result<Address> {
    Address::try_from(b.as_slice()).context("invalid address bytes")
}

fn sql_u128(value: u128) -> BigDecimal {
    BigDecimal::from(BigInt::from(value))
}

fn sql_i128(value: i128) -> BigDecimal {
    BigDecimal::from(BigInt::from(value))
}

fn address_bytes_list(addresses: &[Address]) -> Vec<&[u8]> {
    addresses.iter().map(|address| address.as_slice()).collect()
}

fn decode_pool_rows(rows: Vec<PgRow>) -> Result<Vec<PoolRow>> {
    rows.into_iter().map(PoolRow::try_from).collect()
}

pub async fn get_checkpoint(
    pool: &PgPool,
    chain_id: u64,
    contract: &Address,
) -> Result<Option<u64>> {
    let row = sqlx::query(
        "SELECT block_number FROM pool_indexer_checkpoints WHERE chain_id = $1 AND contract = $2",
    )
    .bind(chain_id.cast_signed())
    .bind(contract.as_slice())
    .fetch_optional(pool)
    .await
    .context("get_checkpoint")?;

    Ok(row.map(|r| r.get::<i64, _>("block_number").cast_unsigned()))
}

pub async fn set_checkpoint(
    executor: impl sqlx::PgExecutor<'_>,
    chain_id: u64,
    contract: &Address,
    block_number: u64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO pool_indexer_checkpoints (chain_id, contract, block_number)
         VALUES ($1, $2, $3)
         ON CONFLICT (chain_id, contract) DO UPDATE SET block_number = EXCLUDED.block_number",
    )
    .bind(chain_id.cast_signed())
    .bind(contract.as_slice())
    .bind(block_number.cast_signed())
    .execute(executor)
    .await
    .context("set_checkpoint")?;
    Ok(())
}

pub async fn insert_pools(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    factory: &Address,
    pools: &[NewPoolData],
) -> Result<()> {
    if pools.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = pools.iter().map(|pool| pool.address.as_slice()).collect();
    let token0s: Vec<&[u8]> = pools.iter().map(|pool| pool.token0.as_slice()).collect();
    let token1s: Vec<&[u8]> = pools.iter().map(|pool| pool.token1.as_slice()).collect();
    let fees: Vec<i32> = pools.iter().map(|pool| pool.fee.cast_signed()).collect();
    let t0_decimals: Vec<Option<i16>> = pools
        .iter()
        .map(|pool| pool.token0_decimals.map(i16::from))
        .collect();
    let t1_decimals: Vec<Option<i16>> = pools
        .iter()
        .map(|pool| pool.token1_decimals.map(i16::from))
        .collect();
    let t0_symbols: Vec<Option<String>> = pools.iter().map(|p| p.token0_symbol.clone()).collect();
    let t1_symbols: Vec<Option<String>> = pools.iter().map(|p| p.token1_symbol.clone()).collect();
    let created_blocks: Vec<i64> = pools
        .iter()
        .map(|pool| pool.created_block.cast_signed())
        .collect();

    sqlx::query(
        "INSERT INTO uniswap_v3_pools
             (chain_id, address, factory, token0, token1, fee, token0_decimals, token1_decimals,
              token0_symbol, token1_symbol, created_block)
         SELECT $1, t.addr, $2, t.t0, t.t1, t.fee, t.t0d, t.t1d, t.t0s, t.t1s, t.cblk
         FROM UNNEST($3::BYTEA[], $4::BYTEA[], $5::BYTEA[], $6::INT4[], $7::INT2[], $8::INT2[],
                     $9::TEXT[], $10::TEXT[], $11::INT8[])
              AS t(addr, t0, t1, fee, t0d, t1d, t0s, t1s, cblk)
         ON CONFLICT (chain_id, address) DO NOTHING",
    )
    .bind(chain_id.cast_signed())
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
    chain_id: u64,
    factory: &Address,
    states: &[PoolStateData],
) -> Result<()> {
    if states.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = states
        .iter()
        .map(|state| state.pool_address.as_slice())
        .collect();
    let block_numbers: Vec<i64> = states
        .iter()
        .map(|state| state.block_number.cast_signed())
        .collect();
    let sqrt_prices: Vec<BigDecimal> = states
        .iter()
        .map(|state| u160_to_big_decimal(&state.sqrt_price_x96))
        .collect();
    let liquidities: Vec<BigDecimal> = states
        .iter()
        .map(|state| sql_u128(state.liquidity))
        .collect();
    let ticks: Vec<i32> = states.iter().map(|state| state.tick).collect();

    sqlx::query(
        "WITH latest AS (
             SELECT DISTINCT ON (addr) addr, blk, sqrt, liq, tick
             FROM UNNEST($3::BYTEA[], $4::INT8[], $5::NUMERIC[], $6::NUMERIC[], $7::INT4[])
                  AS t(addr, blk, sqrt, liq, tick)
             ORDER BY addr, blk DESC
         )
         INSERT INTO uniswap_v3_pool_states
             (chain_id, pool_address, block_number, sqrt_price_x96, liquidity, tick)
         SELECT $1, l.addr, l.blk, l.sqrt, l.liq, l.tick
         FROM latest l
         WHERE EXISTS (
             SELECT 1 FROM uniswap_v3_pools
             WHERE chain_id = $1 AND address = l.addr AND factory = $2
         )
         ON CONFLICT (chain_id, pool_address) DO UPDATE
             SET block_number   = EXCLUDED.block_number,
                 sqrt_price_x96 = EXCLUDED.sqrt_price_x96,
                 liquidity      = EXCLUDED.liquidity,
                 tick           = EXCLUDED.tick",
    )
    .bind(chain_id.cast_signed())
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
    chain_id: u64,
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
        .map(|update| sql_u128(update.liquidity))
        .collect();
    let block_numbers: Vec<i64> = updates
        .iter()
        .map(|update| update.block_number.cast_signed())
        .collect();

    sqlx::query(
        "WITH latest AS (
             SELECT DISTINCT ON (addr) addr, liq, blk
             FROM UNNEST($3::BYTEA[], $4::NUMERIC[], $5::INT8[]) AS t(addr, liq, blk)
             ORDER BY addr, blk DESC
         )
         UPDATE uniswap_v3_pool_states s
         SET liquidity = l.liq, block_number = l.blk
         FROM latest l
         WHERE s.chain_id = $1 AND s.pool_address = l.addr
           AND EXISTS (
               SELECT 1 FROM uniswap_v3_pools p
               WHERE p.chain_id = $1 AND p.address = l.addr AND p.factory = $2
           )",
    )
    .bind(chain_id.cast_signed())
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(liquidities)
    .bind(block_numbers)
    .execute(&mut **tx)
    .await
    .context("batch_update_pool_liquidity")?;
    Ok(())
}

pub async fn batch_update_ticks(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
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
    let delta_values: Vec<BigDecimal> = deltas.iter().map(|delta| sql_i128(delta.delta)).collect();

    sqlx::query(
        "WITH input AS (
             SELECT t.addr, t.tick_idx, SUM(t.delta) AS total_delta
             FROM UNNEST($3::BYTEA[], $4::INT4[], $5::NUMERIC[]) AS t(addr, tick_idx, delta)
             GROUP BY t.addr, t.tick_idx
         ),
         upserted AS (
             INSERT INTO uniswap_v3_ticks (chain_id, pool_address, tick_idx, liquidity_net)
             SELECT $1, i.addr, i.tick_idx, i.total_delta
             FROM input i
             WHERE EXISTS (
                 SELECT 1 FROM uniswap_v3_pools
                 WHERE chain_id = $1 AND address = i.addr AND factory = $2
             )
             ON CONFLICT (chain_id, pool_address, tick_idx) DO UPDATE
                 SET liquidity_net = uniswap_v3_ticks.liquidity_net + EXCLUDED.liquidity_net
             RETURNING chain_id, pool_address, tick_idx, liquidity_net
         )
         DELETE FROM uniswap_v3_ticks ticks
         USING upserted
         WHERE ticks.chain_id   = upserted.chain_id
           AND ticks.pool_address = upserted.pool_address
           AND ticks.tick_idx   = upserted.tick_idx
           AND upserted.liquidity_net = 0",
    )
    .bind(chain_id.cast_signed())
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
    executor: impl sqlx::PgExecutor<'_>,
    chain_id: u64,
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
    let values: Vec<BigDecimal> = ticks.iter().map(|tick| sql_i128(tick.delta)).collect();

    sqlx::query(
        "WITH input AS (
             SELECT t.addr, t.tick_idx, SUM(t.val) AS net
             FROM UNNEST($3::BYTEA[], $4::INT4[], $5::NUMERIC[]) AS t(addr, tick_idx, val)
             GROUP BY t.addr, t.tick_idx
         )
         INSERT INTO uniswap_v3_ticks (chain_id, pool_address, tick_idx, liquidity_net)
         SELECT $1, i.addr, i.tick_idx, i.net
         FROM input i
         WHERE EXISTS (
             SELECT 1 FROM uniswap_v3_pools
             WHERE chain_id = $1 AND address = i.addr AND factory = $2
         )
           AND i.net <> 0
         ON CONFLICT (chain_id, pool_address, tick_idx) DO UPDATE
             SET liquidity_net = EXCLUDED.liquidity_net",
    )
    .bind(chain_id.cast_signed())
    .bind(factory.as_slice())
    .bind(addresses)
    .bind(tick_idxs)
    .bind(values)
    .execute(executor)
    .await
    .context("batch_seed_ticks")?;
    Ok(())
}

/// Deletes ticks for all pools owned by `factory` on `chain_id`. Used by the
/// subgraph seeder to clear stale state before reseeding. Scoped to this
/// factory so a reseed on one factory doesn't wipe another's ticks.
pub async fn delete_ticks_for_factory(
    executor: impl sqlx::PgExecutor<'_>,
    chain_id: u64,
    factory: &Address,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM uniswap_v3_ticks t
         USING uniswap_v3_pools p
         WHERE t.chain_id     = $1
           AND p.chain_id     = $1
           AND p.address      = t.pool_address
           AND p.factory      = $2",
    )
    .bind(chain_id.cast_signed())
    .bind(factory.as_slice())
    .execute(executor)
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
pub async fn get_pools(
    pool: &PgPool,
    chain_id: u64,
    cursor: Option<Vec<u8>>,
    limit: u64,
) -> Result<Vec<PoolRow>> {
    let rows = sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s
             ON s.chain_id = p.chain_id AND s.pool_address = p.address
         WHERE p.chain_id = $1
           AND ($2::BYTEA IS NULL OR p.address > $2)
         ORDER BY p.address
         LIMIT $3",
    )
    .bind(chain_id.cast_signed())
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

/// Upper bound on ticks returned per pool query. Sized ~3× the largest known
/// mainnet pool: USDC/WETH 0.05% (0x88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640)
/// had 1533 active ticks on 2026-04-22. Callers that hit this limit get a
/// `warn_truncated` log; bump if that starts firing on real pools.
pub const MAX_TICKS_PER_POOL: u32 = 5_000;

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
pub async fn get_pools_by_ids(
    pool: &PgPool,
    chain_id: u64,
    addresses: &[Address],
) -> Result<Vec<PoolRow>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s
             ON s.chain_id = p.chain_id AND s.pool_address = p.address
         WHERE p.chain_id = $1
           AND p.address = ANY($2)
         ORDER BY p.address",
    )
    .bind(chain_id.cast_signed())
    .bind(address_bytes_list(addresses))
    .fetch_all(pool)
    .await
    .context("get_pools_by_ids")?;

    decode_pool_rows(rows)
}

/// Fetches ticks for multiple pools in one query, capped at
/// [`MAX_TICKS_PER_POOL`] per pool. Uses a `LATERAL` join so each pool's
/// limit is applied individually via the PK prefix index — a flat
/// `WHERE pool_address = ANY($2)` with a single outer `LIMIT` could starve
/// later pools when one has many ticks. Rows are ordered by
/// `(pool_address, tick_idx)` so callers can group in a single pass.
pub async fn get_ticks_for_pools(
    pool: &PgPool,
    chain_id: u64,
    addresses: &[Address],
) -> Result<Vec<PoolTickRow>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT t.pool_address, t.tick_idx, t.liquidity_net
         FROM UNNEST($2::BYTEA[]) AS p(addr)
         JOIN LATERAL (
             SELECT pool_address, tick_idx, liquidity_net
             FROM uniswap_v3_ticks
             WHERE chain_id = $1 AND pool_address = p.addr
             ORDER BY tick_idx
             LIMIT $3
         ) t ON TRUE
         ORDER BY t.pool_address, t.tick_idx",
    )
    .bind(chain_id.cast_signed())
    .bind(address_bytes_list(addresses))
    .bind(i64::from(MAX_TICKS_PER_POOL))
    .fetch_all(pool)
    .await
    .context("get_ticks_for_pools")?;

    let out: Vec<PoolTickRow> = rows
        .into_iter()
        .map(|r| {
            Ok::<_, anyhow::Error>(PoolTickRow {
                pool_address: bytes_to_addr(r.get("pool_address"))?,
                tick_idx: r.get("tick_idx"),
                liquidity_net: r.get("liquidity_net"),
            })
        })
        .collect::<Result<_>>()?;
    warn_on_truncated_pools(&out);
    Ok(out)
}

pub async fn get_ticks(
    pool: &PgPool,
    chain_id: u64,
    pool_address: &Address,
) -> Result<Vec<TickRow>> {
    let ticks: Vec<TickRow> = sqlx::query(
        "SELECT tick_idx, liquidity_net
         FROM uniswap_v3_ticks
         WHERE chain_id = $1
           AND pool_address = $2
         ORDER BY tick_idx
         LIMIT $3",
    )
    .bind(chain_id.cast_signed())
    .bind(pool_address.as_slice())
    .bind(i64::from(MAX_TICKS_PER_POOL))
    .fetch_all(pool)
    .await
    .context("get_ticks")?
    .into_iter()
    .map(|r| TickRow {
        tick_idx: r.get("tick_idx"),
        liquidity_net: r.get("liquidity_net"),
    })
    .collect();

    if ticks.len() >= MAX_TICKS_PER_POOL as usize {
        warn_truncated(pool_address);
    }
    Ok(ticks)
}

fn warn_on_truncated_pools(rows: &[PoolTickRow]) {
    let mut tick_count: std::collections::HashMap<&Address, usize> =
        std::collections::HashMap::new();
    for row in rows {
        *tick_count.entry(&row.pool_address).or_default() += 1;
    }
    for (addr, count) in tick_count {
        if count >= MAX_TICKS_PER_POOL as usize {
            warn_truncated(addr);
        }
    }
}

fn warn_truncated(pool: &Address) {
    tracing::warn!(
        %pool,
        limit = MAX_TICKS_PER_POOL,
        "tick query hit MAX_TICKS_PER_POOL limit; results may be truncated",
    );
}

/// Returns all distinct token addresses that have no symbol recorded yet.
pub async fn get_tokens_missing_symbols(pool: &PgPool, chain_id: u64) -> Result<Vec<Address>> {
    let rows = sqlx::query(
        "SELECT DISTINCT token FROM (
             SELECT token0 AS token FROM uniswap_v3_pools
             WHERE chain_id = $1 AND token0_symbol IS NULL
             UNION
             SELECT token1 AS token FROM uniswap_v3_pools
             WHERE chain_id = $1 AND token1_symbol IS NULL
         ) t",
    )
    .bind(chain_id.cast_signed())
    .fetch_all(pool)
    .await
    .context("get_tokens_missing_symbols")?;

    rows.into_iter()
        .map(|r| bytes_to_addr(r.get("token")))
        .collect()
}

/// Returns all distinct token addresses that have no decimals recorded yet.
pub async fn get_tokens_missing_decimals(pool: &PgPool, chain_id: u64) -> Result<Vec<Address>> {
    let rows = sqlx::query(
        "SELECT DISTINCT token FROM (
             SELECT token0 AS token FROM uniswap_v3_pools
             WHERE chain_id = $1 AND token0_decimals IS NULL
             UNION
             SELECT token1 AS token FROM uniswap_v3_pools
             WHERE chain_id = $1 AND token1_decimals IS NULL
         ) t",
    )
    .bind(chain_id.cast_signed())
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
    pool: &PgPool,
    chain_id: u64,
    entries: &[(Address, i16)],
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let tokens: Vec<&[u8]> = entries.iter().map(|(t, _)| t.as_slice()).collect();
    let decimals: Vec<i16> = entries.iter().map(|(_, d)| *d).collect();

    sqlx::query(
        "WITH input AS (
             SELECT * FROM UNNEST($2::BYTEA[], $3::INT2[]) AS t(tok, dec)
         ),
         update_t0 AS (
             UPDATE uniswap_v3_pools p
             SET token0_decimals = i.dec
             FROM input i
             WHERE p.chain_id = $1
               AND p.token0 = i.tok
               AND p.token0_decimals IS NULL
             RETURNING 1
         )
         UPDATE uniswap_v3_pools p
         SET token1_decimals = i.dec
         FROM input i
         WHERE p.chain_id = $1
           AND p.token1 = i.tok
           AND p.token1_decimals IS NULL",
    )
    .bind(chain_id.cast_signed())
    .bind(tokens)
    .bind(decimals)
    .execute(pool)
    .await
    .context("batch_set_token_decimals")?;

    Ok(())
}

/// Batched update of `token0_symbol` / `token1_symbol` for every pool
/// containing one of the provided tokens. Pass `""` for entries that were
/// "tried, failed" so the next backfill pass's `IS NULL` filter skips them.
/// See [`batch_set_token_decimals`] for the writeable-CTE rationale.
pub async fn batch_set_token_symbols(
    pool: &PgPool,
    chain_id: u64,
    entries: &[(Address, String)],
) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let tokens: Vec<&[u8]> = entries.iter().map(|(t, _)| t.as_slice()).collect();
    let symbols: Vec<&str> = entries.iter().map(|(_, s)| s.as_str()).collect();

    sqlx::query(
        "WITH input AS (
             SELECT * FROM UNNEST($2::BYTEA[], $3::TEXT[]) AS t(tok, sym)
         ),
         update_t0 AS (
             UPDATE uniswap_v3_pools p
             SET token0_symbol = i.sym
             FROM input i
             WHERE p.chain_id = $1
               AND p.token0 = i.tok
               AND p.token0_symbol IS NULL
             RETURNING 1
         )
         UPDATE uniswap_v3_pools p
         SET token1_symbol = i.sym
         FROM input i
         WHERE p.chain_id = $1
           AND p.token1 = i.tok
           AND p.token1_symbol IS NULL",
    )
    .bind(chain_id.cast_signed())
    .bind(tokens)
    .bind(symbols)
    .execute(pool)
    .await
    .context("batch_set_token_symbols")?;

    Ok(())
}

pub async fn get_latest_indexed_block(pool: &PgPool, chain_id: u64) -> Result<Option<u64>> {
    let row = sqlx::query(
        "SELECT MAX(block_number) AS max_block FROM pool_indexer_checkpoints WHERE chain_id = $1",
    )
    .bind(chain_id.cast_signed())
    .fetch_one(pool)
    .await
    .context("get_latest_indexed_block")?;

    Ok(row
        .get::<Option<i64>, _>("max_block")
        .map(|b| b.cast_unsigned()))
}
