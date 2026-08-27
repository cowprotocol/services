pub mod balancer_v2;
pub mod uniswap_v3;

use {
    alloy_primitives::{Address, B256},
    anyhow::{Context, Result},
    sqlx::{PgPool, Postgres, Row, Transaction},
    std::collections::BTreeSet,
};

/// Decodes a Postgres `BYTEA` column into an [`Address`].
pub(crate) fn bytes_to_addr(b: Vec<u8>) -> Result<Address> {
    Address::try_from(b.as_slice()).context("invalid address bytes")
}

/// Decodes a Postgres `BYTEA` column into a 32-byte [`B256`] Balancer pool id.
pub(crate) fn bytes_to_b256(b: &[u8]) -> Result<B256> {
    B256::try_from(b).context("invalid pool_id bytes")
}

/// Highest block scanned for a factory's `PoolCreated` events. Shared by both
/// indexers: `pool_indexer_checkpoints` is keyed by factory address, which is
/// unique across protocols, so their rows never collide.
pub async fn get_checkpoint(pool: &PgPool, factory: &Address) -> Result<Option<u64>> {
    let row = sqlx::query(
        "SELECT block_number FROM pool_indexer_checkpoints WHERE contract_address = $1",
    )
    .bind(factory.as_slice())
    .fetch_optional(pool)
    .await
    .context("get_checkpoint")?;

    Ok(row.map(|r| r.get::<i64, _>("block_number").cast_unsigned()))
}

pub async fn set_checkpoint(
    tx: &mut Transaction<'_, Postgres>,
    factory: &Address,
    block_number: u64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO pool_indexer_checkpoints (contract_address, block_number)
         VALUES ($1, $2)
         ON CONFLICT (contract_address) DO UPDATE SET block_number = EXCLUDED.block_number",
    )
    .bind(factory.as_slice())
    .bind(block_number.cast_signed())
    .execute(&mut **tx)
    .await
    .context("set_checkpoint")?;
    Ok(())
}

/// The block every configured factory is indexed through, so every served pool
/// is current at least to here. Scoped to `factories` so a decommissioned
/// factory's leftover checkpoint row can't pin the value.
pub async fn get_latest_indexed_block(
    pool: &PgPool,
    factories: &BTreeSet<Address>,
) -> Result<Option<u64>> {
    let factories: Vec<&[u8]> = factories.iter().map(|f| f.as_slice()).collect();
    let row = sqlx::query(
        "SELECT MIN(block_number) AS block FROM pool_indexer_checkpoints
         WHERE contract_address = ANY($1)",
    )
    .bind(factories)
    .fetch_one(pool)
    .await
    .context("get_latest_indexed_block")?;

    Ok(row
        .get::<Option<i64>, _>("block")
        .map(|b| b.cast_unsigned()))
}
