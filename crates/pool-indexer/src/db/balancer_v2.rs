use {
    crate::{db::bytes_to_addr, indexer::balancer_v2::NewBalancerPool},
    alloy_primitives::Address,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    sqlx::{PgPool, Postgres, Row, Transaction},
};

/// Inserts discovered pools and their tokens. Pools are written before tokens
/// to satisfy the FK; both `ON CONFLICT DO NOTHING` so re-indexing a pool is a
/// no-op.
pub async fn insert_pools(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    pools: &[NewBalancerPool],
) -> Result<()> {
    if pools.is_empty() {
        return Ok(());
    }

    let mut pool_ids: Vec<&[u8]> = Vec::with_capacity(pools.len());
    let mut addresses: Vec<&[u8]> = Vec::with_capacity(pools.len());
    let mut pool_types: Vec<&str> = Vec::with_capacity(pools.len());
    let mut created_blocks: Vec<i64> = Vec::with_capacity(pools.len());
    let mut tok_pool_ids: Vec<&[u8]> = Vec::new();
    let mut positions: Vec<i32> = Vec::new();
    let mut tokens: Vec<&[u8]> = Vec::new();
    let mut decimals: Vec<Option<i16>> = Vec::new();
    let mut weights: Vec<Option<BigDecimal>> = Vec::new();
    for pool in pools {
        pool_ids.push(pool.pool_id.as_slice());
        addresses.push(pool.address.as_slice());
        pool_types.push(pool.pool_type.as_str());
        created_blocks.push(pool.created_block.cast_signed());
        for token in &pool.tokens {
            tok_pool_ids.push(pool.pool_id.as_slice());
            positions.push(i32::try_from(token.position).unwrap_or(i32::MAX));
            tokens.push(token.address.as_slice());
            decimals.push(token.decimals.map(i16::from));
            weights.push(token.weight.clone());
        }
    }

    sqlx::query(
        "INSERT INTO balancer_v2_pools (pool_id, address, factory, pool_type, created_block)
         SELECT t.pid, t.addr, $1, t.ptype, t.cblk
         FROM UNNEST($2::BYTEA[], $3::BYTEA[], $4::TEXT[], $5::INT8[])
              AS t(pid, addr, ptype, cblk)
         ON CONFLICT (pool_id) DO NOTHING",
    )
    .bind(factory.as_slice())
    .bind(pool_ids)
    .bind(addresses)
    .bind(pool_types)
    .bind(created_blocks)
    .execute(&mut **tx)
    .await
    .context("insert balancer pools")?;

    sqlx::query(
        "INSERT INTO balancer_v2_pool_tokens (pool_id, position, token, decimals, weight)
         SELECT t.pid, t.pos, t.tok, t.dec, t.wgt
         FROM UNNEST($1::BYTEA[], $2::INT4[], $3::BYTEA[], $4::INT2[], $5::NUMERIC[])
              AS t(pid, pos, tok, dec, wgt)
         ON CONFLICT (pool_id, position) DO NOTHING",
    )
    .bind(tok_pool_ids)
    .bind(positions)
    .bind(tokens)
    .bind(decimals)
    .bind(weights)
    .execute(&mut **tx)
    .await
    .context("insert balancer pool tokens")?;

    Ok(())
}

/// Distinct token addresses with no `decimals` recorded yet.
pub async fn get_tokens_missing_decimals(pool: &PgPool) -> Result<Vec<Address>> {
    let rows =
        sqlx::query("SELECT DISTINCT token FROM balancer_v2_pool_tokens WHERE decimals IS NULL")
            .fetch_all(pool)
            .await
            .context("get_tokens_missing_decimals")?;

    rows.into_iter()
        .map(|r| bytes_to_addr(r.get("token")))
        .collect()
}

/// Sets `decimals` for every token row matching one of the inputs. Pass `-1`
/// for "tried, failed" so the next backfill's `IS NULL` filter still skips it.
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
        "UPDATE balancer_v2_pool_tokens p
         SET decimals = i.dec
         FROM UNNEST($1::BYTEA[], $2::INT2[]) AS i(tok, dec)
         WHERE p.token = i.tok AND p.decimals IS NULL",
    )
    .bind(tokens)
    .bind(decimals)
    .execute(&mut **tx)
    .await
    .context("batch_set_token_decimals")?;
    Ok(())
}
