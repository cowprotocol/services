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
    tx: &mut Transaction<'_, Postgres>,
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
    .execute(&mut **tx)
    .await
    .context("set_checkpoint")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_pool(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    address: &Address,
    token0: &Address,
    token1: &Address,
    fee: u32,
    token0_decimals: Option<u8>,
    token1_decimals: Option<u8>,
    created_block: u64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO uniswap_v3_pools
             (chain_id, address, token0, token1, fee, token0_decimals, token1_decimals, \
         created_block)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (chain_id, address) DO NOTHING",
    )
    .bind(chain_id.cast_signed())
    .bind(address.as_slice())
    .bind(token0.as_slice())
    .bind(token1.as_slice())
    .bind(fee.cast_signed())
    .bind(token0_decimals.map(i16::from))
    .bind(token1_decimals.map(i16::from))
    .bind(created_block.cast_signed())
    .execute(&mut **tx)
    .await
    .context("insert_pool")?;
    Ok(())
}

pub async fn upsert_pool_state(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    pool_address: &Address,
    block_number: u64,
    sqrt_price_x96: alloy_primitives::aliases::U160,
    liquidity: u128,
    tick: i32,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO uniswap_v3_pool_states
             (chain_id, pool_address, block_number, sqrt_price_x96, liquidity, tick)
         SELECT $1, $2, $3, $4, $5, $6
         WHERE EXISTS (SELECT 1 FROM uniswap_v3_pools WHERE chain_id = $1 AND address = $2)
         ON CONFLICT (chain_id, pool_address) DO UPDATE
             SET block_number   = EXCLUDED.block_number,
                 sqrt_price_x96 = EXCLUDED.sqrt_price_x96,
                 liquidity      = EXCLUDED.liquidity,
                 tick           = EXCLUDED.tick",
    )
    .bind(chain_id.cast_signed())
    .bind(pool_address.as_slice())
    .bind(block_number.cast_signed())
    .bind(u160_to_big_decimal(&sqrt_price_x96))
    .bind(BigDecimal::from(BigInt::from(liquidity)))
    .bind(tick)
    .execute(&mut **tx)
    .await
    .context("upsert_pool_state")?;
    Ok(())
}

pub async fn update_pool_liquidity(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    pool_address: &Address,
    block_number: u64,
    liquidity: u128,
) -> Result<()> {
    sqlx::query(
        "UPDATE uniswap_v3_pool_states
         SET liquidity = $3, block_number = $4
         WHERE chain_id = $1 AND pool_address = $2",
    )
    .bind(chain_id.cast_signed())
    .bind(pool_address.as_slice())
    .bind(BigDecimal::from(BigInt::from(liquidity)))
    .bind(block_number.cast_signed())
    .execute(&mut **tx)
    .await
    .context("update_pool_liquidity")?;
    Ok(())
}

/// Applies a signed delta to a tick's `liquidity_net`. Rows that reach zero
/// are pruned (Uniswap V3 convention).
pub async fn update_tick_liquidity_net(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    pool_address: &Address,
    tick_idx: i32,
    delta: i128,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO uniswap_v3_ticks (chain_id, pool_address, tick_idx, liquidity_net)
         SELECT $1, $2, $3, $4
         WHERE EXISTS (SELECT 1 FROM uniswap_v3_pools WHERE chain_id = $1 AND address = $2)
         ON CONFLICT (chain_id, pool_address, tick_idx) DO UPDATE
             SET liquidity_net = uniswap_v3_ticks.liquidity_net + EXCLUDED.liquidity_net",
    )
    .bind(chain_id.cast_signed())
    .bind(pool_address.as_slice())
    .bind(tick_idx)
    .bind(BigDecimal::from(BigInt::from(delta)))
    .execute(&mut **tx)
    .await
    .context("update_tick_liquidity_net upsert")?;

    sqlx::query(
        "DELETE FROM uniswap_v3_ticks
         WHERE chain_id = $1 AND pool_address = $2 AND tick_idx = $3 AND liquidity_net = 0",
    )
    .bind(chain_id.cast_signed())
    .bind(pool_address.as_slice())
    .bind(tick_idx)
    .execute(&mut **tx)
    .await
    .context("update_tick_liquidity_net prune")?;

    Ok(())
}

pub async fn batch_insert_pools(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    pools: &[NewPoolData],
) -> Result<()> {
    if pools.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = pools.iter().map(|p| p.address.as_slice()).collect();
    let token0s: Vec<&[u8]> = pools.iter().map(|p| p.token0.as_slice()).collect();
    let token1s: Vec<&[u8]> = pools.iter().map(|p| p.token1.as_slice()).collect();
    let fees: Vec<i32> = pools.iter().map(|p| p.fee.cast_signed()).collect();
    let t0_decimals: Vec<Option<i16>> = pools
        .iter()
        .map(|p| p.token0_decimals.map(i16::from))
        .collect();
    let t1_decimals: Vec<Option<i16>> = pools
        .iter()
        .map(|p| p.token1_decimals.map(i16::from))
        .collect();
    let t0_symbols: Vec<Option<String>> = pools.iter().map(|p| p.token0_symbol.clone()).collect();
    let t1_symbols: Vec<Option<String>> = pools.iter().map(|p| p.token1_symbol.clone()).collect();
    let created_blocks: Vec<i64> = pools
        .iter()
        .map(|p| p.created_block.cast_signed())
        .collect();

    sqlx::query(
        "INSERT INTO uniswap_v3_pools
             (chain_id, address, token0, token1, fee, token0_decimals, token1_decimals,
              token0_symbol, token1_symbol, created_block)
         SELECT $1, t.addr, t.t0, t.t1, t.fee, t.t0d, t.t1d, t.t0s, t.t1s, t.cblk
         FROM UNNEST($2::BYTEA[], $3::BYTEA[], $4::BYTEA[], $5::INT4[], $6::INT2[], $7::INT2[],
                     $8::TEXT[], $9::TEXT[], $10::INT8[])
              AS t(addr, t0, t1, fee, t0d, t1d, t0s, t1s, cblk)
         ON CONFLICT (chain_id, address) DO NOTHING",
    )
    .bind(chain_id.cast_signed())
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
    .context("batch_insert_pools")?;
    Ok(())
}

pub async fn batch_upsert_pool_states(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    states: &[PoolStateData],
) -> Result<()> {
    if states.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = states.iter().map(|s| s.pool_address.as_slice()).collect();
    let block_numbers: Vec<i64> = states
        .iter()
        .map(|s| s.block_number.cast_signed())
        .collect();
    let sqrt_prices: Vec<BigDecimal> = states
        .iter()
        .map(|s| u160_to_big_decimal(&s.sqrt_price_x96))
        .collect();
    let liquidities: Vec<BigDecimal> = states
        .iter()
        .map(|s| BigDecimal::from(BigInt::from(s.liquidity)))
        .collect();
    let ticks: Vec<i32> = states.iter().map(|s| s.tick).collect();

    sqlx::query(
        "WITH latest AS (
             SELECT DISTINCT ON (addr) addr, blk, sqrt, liq, tick
             FROM UNNEST($2::BYTEA[], $3::INT8[], $4::NUMERIC[], $5::NUMERIC[], $6::INT4[])
                  AS t(addr, blk, sqrt, liq, tick)
             ORDER BY addr, blk DESC
         )
         INSERT INTO uniswap_v3_pool_states
             (chain_id, pool_address, block_number, sqrt_price_x96, liquidity, tick)
         SELECT $1, l.addr, l.blk, l.sqrt, l.liq, l.tick
         FROM latest l
         WHERE EXISTS (SELECT 1 FROM uniswap_v3_pools WHERE chain_id = $1 AND address = l.addr)
         ON CONFLICT (chain_id, pool_address) DO UPDATE
             SET block_number   = EXCLUDED.block_number,
                 sqrt_price_x96 = EXCLUDED.sqrt_price_x96,
                 liquidity      = EXCLUDED.liquidity,
                 tick           = EXCLUDED.tick",
    )
    .bind(chain_id.cast_signed())
    .bind(addresses)
    .bind(block_numbers)
    .bind(sqrt_prices)
    .bind(liquidities)
    .bind(ticks)
    .execute(&mut **tx)
    .await
    .context("batch_upsert_pool_states")?;
    Ok(())
}

pub async fn batch_update_pool_liquidity(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
    updates: &[LiquidityUpdateData],
) -> Result<()> {
    if updates.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = updates.iter().map(|u| u.pool_address.as_slice()).collect();
    let liquidities: Vec<BigDecimal> = updates
        .iter()
        .map(|u| BigDecimal::from(BigInt::from(u.liquidity)))
        .collect();
    let block_numbers: Vec<i64> = updates
        .iter()
        .map(|u| u.block_number.cast_signed())
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
         WHERE s.chain_id = $1 AND s.pool_address = l.addr",
    )
    .bind(chain_id.cast_signed())
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
    deltas: &[TickDeltaData],
) -> Result<()> {
    if deltas.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = deltas.iter().map(|d| d.pool_address.as_slice()).collect();
    let tick_idxs: Vec<i32> = deltas.iter().map(|d| d.tick_idx).collect();
    let delta_values: Vec<BigDecimal> = deltas
        .iter()
        .map(|d| BigDecimal::from(BigInt::from(d.delta)))
        .collect();

    sqlx::query(
        "WITH input AS (
             SELECT t.addr, t.tick_idx, SUM(t.delta) AS total_delta
             FROM UNNEST($2::BYTEA[], $3::INT4[], $4::NUMERIC[]) AS t(addr, tick_idx, delta)
             GROUP BY t.addr, t.tick_idx
         ),
         upserted AS (
             INSERT INTO uniswap_v3_ticks (chain_id, pool_address, tick_idx, liquidity_net)
             SELECT $1, i.addr, i.tick_idx, i.total_delta
             FROM input i
             WHERE EXISTS (SELECT 1 FROM uniswap_v3_pools WHERE chain_id = $1 AND address = i.addr)
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
    chain_id: u64,
    ticks: &[TickDeltaData],
) -> Result<()> {
    if ticks.is_empty() {
        return Ok(());
    }
    let addresses: Vec<&[u8]> = ticks.iter().map(|d| d.pool_address.as_slice()).collect();
    let tick_idxs: Vec<i32> = ticks.iter().map(|d| d.tick_idx).collect();
    let values: Vec<BigDecimal> = ticks
        .iter()
        .map(|d| BigDecimal::from(BigInt::from(d.delta)))
        .collect();

    sqlx::query(
        "WITH input AS (
             SELECT t.addr, t.tick_idx, SUM(t.val) AS net
             FROM UNNEST($2::BYTEA[], $3::INT4[], $4::NUMERIC[]) AS t(addr, tick_idx, val)
             GROUP BY t.addr, t.tick_idx
         )
         INSERT INTO uniswap_v3_ticks (chain_id, pool_address, tick_idx, liquidity_net)
         SELECT $1, i.addr, i.tick_idx, i.net
         FROM input i
         WHERE EXISTS (SELECT 1 FROM uniswap_v3_pools WHERE chain_id = $1 AND address = i.addr)
           AND i.net <> 0
         ON CONFLICT (chain_id, pool_address, tick_idx) DO UPDATE
             SET liquidity_net = EXCLUDED.liquidity_net",
    )
    .bind(chain_id.cast_signed())
    .bind(addresses)
    .bind(tick_idxs)
    .bind(values)
    .execute(&mut **tx)
    .await
    .context("batch_seed_ticks")?;
    Ok(())
}

pub async fn delete_ticks_for_chain(
    tx: &mut Transaction<'_, Postgres>,
    chain_id: u64,
) -> Result<()> {
    sqlx::query("DELETE FROM uniswap_v3_ticks WHERE chain_id = $1")
        .bind(chain_id.cast_signed())
        .execute(&mut **tx)
        .await
        .context("delete_ticks_for_chain")?;
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
            token0_decimals: r
                .get::<Option<i16>, _>("token0_decimals")
                .map(|d| u8::try_from(d).unwrap_or(0)),
            token1_decimals: r
                .get::<Option<i16>, _>("token1_decimals")
                .map(|d| u8::try_from(d).unwrap_or(0)),
            token0_symbol: r.get("token0_symbol"),
            token1_symbol: r.get("token1_symbol"),
            sqrt_price_x96: r.get("sqrt_price_x96"),
            liquidity: r.get("liquidity"),
            tick: r.get("tick"),
        })
    }
}

/// Fetches a page of pools ordered by address with their current state.
pub async fn get_pools(pool: &PgPool, chain_id: u64, limit: i64) -> Result<Vec<PoolRow>> {
    sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s
             ON s.chain_id = p.chain_id AND s.pool_address = p.address
         WHERE p.chain_id = $1
         ORDER BY p.address
         LIMIT $2",
    )
    .bind(chain_id.cast_signed())
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("get_pools")?
    .into_iter()
    .map(PoolRow::try_from)
    .collect()
}

/// Fetches the next page of pools after `cursor` address (keyset pagination).
pub async fn get_pools_after(
    pool: &PgPool,
    chain_id: u64,
    cursor: Vec<u8>,
    limit: i64,
) -> Result<Vec<PoolRow>> {
    sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s
             ON s.chain_id = p.chain_id AND s.pool_address = p.address
         WHERE p.chain_id = $1
           AND p.address > $2
         ORDER BY p.address
         LIMIT $3",
    )
    .bind(chain_id.cast_signed())
    .bind(cursor)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("get_pools_after")?
    .into_iter()
    .map(PoolRow::try_from)
    .collect()
}

pub struct TickRow {
    pub tick_idx: i32,
    pub liquidity_net: BigDecimal,
}

/// Maximum number of ticks returned per pool query (safety bound).
const MAX_TICKS_PER_POOL: i64 = 10_000;

pub async fn get_ticks(
    pool: &PgPool,
    chain_id: u64,
    pool_address: &Address,
) -> Result<Vec<TickRow>> {
    sqlx::query(
        "SELECT tick_idx, liquidity_net
         FROM uniswap_v3_ticks
         WHERE chain_id = $1
           AND pool_address = $2
         ORDER BY tick_idx
         LIMIT $3",
    )
    .bind(chain_id.cast_signed())
    .bind(pool_address.as_slice())
    .bind(MAX_TICKS_PER_POOL)
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

/// Searches pools by a single token symbol (partial, case-insensitive), ordered
/// by liquidity descending.
pub async fn search_pools_by_token(
    pool: &PgPool,
    chain_id: u64,
    token: &str,
) -> Result<Vec<PoolRow>> {
    let pattern = format!("%{}%", token.to_lowercase());
    sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s
             ON s.chain_id = p.chain_id AND s.pool_address = p.address
         WHERE p.chain_id = $1
           AND (LOWER(p.token0_symbol) LIKE $2 OR LOWER(p.token1_symbol) LIKE $2)
         ORDER BY s.liquidity DESC",
    )
    .bind(chain_id.cast_signed())
    .bind(&pattern)
    .fetch_all(pool)
    .await
    .context("search_pools_by_token")?
    .into_iter()
    .map(PoolRow::try_from)
    .collect()
}

/// Searches pools matching a pair of token symbols (partial, case-insensitive,
/// order-independent), ordered by liquidity descending.
pub async fn search_pools_by_pair(
    pool: &PgPool,
    chain_id: u64,
    token0: &str,
    token1: &str,
) -> Result<Vec<PoolRow>> {
    let t0 = format!("%{}%", token0.to_lowercase());
    let t1 = format!("%{}%", token1.to_lowercase());
    sqlx::query(
        "SELECT p.address, p.token0, p.token1, p.fee,
                p.token0_decimals, p.token1_decimals,
                p.token0_symbol, p.token1_symbol,
                s.sqrt_price_x96, s.liquidity, s.tick
         FROM uniswap_v3_pools p
         JOIN uniswap_v3_pool_states s
             ON s.chain_id = p.chain_id AND s.pool_address = p.address
         WHERE p.chain_id = $1
           AND (
               (LOWER(p.token0_symbol) LIKE $2 AND LOWER(p.token1_symbol) LIKE $3)
               OR (LOWER(p.token1_symbol) LIKE $2 AND LOWER(p.token0_symbol) LIKE $3)
           )
         ORDER BY s.liquidity DESC",
    )
    .bind(chain_id.cast_signed())
    .bind(&t0)
    .bind(&t1)
    .fetch_all(pool)
    .await
    .context("search_pools_by_pair")?
    .into_iter()
    .map(PoolRow::try_from)
    .collect()
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

/// Updates `token0_symbol` / `token1_symbol` for all pools containing `token`.
pub async fn set_token_symbol(
    pool: &PgPool,
    chain_id: u64,
    token: &Address,
    symbol: &str,
) -> Result<()> {
    let mut tx = pool.begin().await.context("set_token_symbol begin")?;

    sqlx::query(
        "UPDATE uniswap_v3_pools SET token0_symbol = $3
         WHERE chain_id = $1 AND token0 = $2 AND token0_symbol IS NULL",
    )
    .bind(chain_id.cast_signed())
    .bind(token.as_slice())
    .bind(symbol)
    .execute(&mut *tx)
    .await
    .context("set_token_symbol token0")?;

    sqlx::query(
        "UPDATE uniswap_v3_pools SET token1_symbol = $3
         WHERE chain_id = $1 AND token1 = $2 AND token1_symbol IS NULL",
    )
    .bind(chain_id.cast_signed())
    .bind(token.as_slice())
    .bind(symbol)
    .execute(&mut *tx)
    .await
    .context("set_token_symbol token1")?;

    tx.commit().await.context("set_token_symbol commit")?;
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
