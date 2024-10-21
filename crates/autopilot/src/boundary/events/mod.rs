use {
    anyhow::{Context, Result},
    sqlx::PgPool,
};

pub mod settlement;

pub async fn write_last_block_to_db(db: &PgPool, last_block: u64, index_name: &str) -> Result<()> {
    let mut ex = db.acquire().await?;
    database::last_processed_blocks::update(
        &mut ex,
        index_name,
        i64::try_from(last_block).context("new value of counter is not i64")?,
    )
    .await?;
    Ok(())
}

pub async fn read_last_block_from_db(db: &PgPool, index_name: &str) -> Result<u64> {
    let mut ex = db.acquire().await?;
    database::last_processed_blocks::fetch(&mut ex, index_name)
        .await?
        .unwrap_or_default()
        .try_into()
        .context("last block is not u64")
}
