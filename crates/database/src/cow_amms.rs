use {
    crate::{Address, PgTransaction},
    sqlx::{Executor, PgConnection, QueryBuilder},
    tracing::instrument,
};

/// Represents a CoW AMM stored in the database
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct CowAmm {
    pub address: Address,
    pub helper_address: Address,
    pub tradeable_tokens: Vec<Address>,
    pub block_number: i64,
}

/// Insert or update multiple CoW AMMs in the database using batch insert
#[instrument(skip_all)]
pub async fn upsert_batched(
    ex: &mut PgTransaction<'_>,
    cow_amms: &[CowAmm],
) -> Result<(), sqlx::Error> {
    if cow_amms.is_empty() {
        return Ok(());
    }

    const BATCH_SIZE: usize = 200;

    for chunk in cow_amms.chunks(BATCH_SIZE) {
        upsert(ex, chunk).await?;
    }

    Ok(())
}

/// Insert or update a batch of CoW AMMs in the database
#[instrument(skip_all)]
async fn upsert(ex: &mut PgConnection, cow_amms: &[CowAmm]) -> Result<(), sqlx::Error> {
    const QUERY: &str = "INSERT INTO cow_amms (address, tradeable_tokens, block_number) ";
    const CONFLICT_CLAUSE: &str = r#"
ON CONFLICT (address)
DO UPDATE SET
    tradeable_tokens = EXCLUDED.tradeable_tokens,
    block_number     = EXCLUDED.block_number
    "#;

    let mut query_builder = QueryBuilder::new(QUERY);

    query_builder.push_values(cow_amms, |mut builder, cow_amm| {
        builder
            .push_bind(cow_amm.address)
            .push_bind(cow_amm.tradeable_tokens.clone())
            .push_bind(cow_amm.block_number);
    });
    query_builder.push(CONFLICT_CLAUSE);
    query_builder.build().execute(ex).await?;

    Ok(())
}

/// Fetch all CoW AMMs for a specific helper contract
#[instrument(skip_all)]
pub async fn fetch_by_helper_address(
    ex: &mut PgConnection,
    address: &Address,
) -> Result<Vec<CowAmm>, sqlx::Error> {
    const QUERY: &str = "SELECT * FROM cow_amms WHERE helper_address = $1";

    let cow_amms = sqlx::query_as(QUERY).bind(address).fetch_all(ex).await?;

    Ok(cow_amms)
}

/// Delete CoW AMMs by their addresses
#[instrument(skip_all)]
pub async fn delete_by_blocks(ex: &mut PgConnection, blocks: &[i64]) -> Result<(), sqlx::Error> {
    if blocks.is_empty() {
        return Ok(());
    }

    const QUERY: &str = r#"
DELETE FROM cow_amms
WHERE block_number = ANY($1);
    "#;

    ex.execute(sqlx::query(QUERY).bind(blocks)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_cow_amm_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let address = ByteArray([1u8; 20]);
        let cow_amm = CowAmm {
            address,
            helper_address: address,
            block_number: 1,
            tradeable_tokens: vec![ByteArray([1u8; 20]), ByteArray([2u8; 20])],
        };

        // Test upsert
        upsert(&mut db, std::slice::from_ref(&cow_amm))
            .await
            .unwrap();

        // Test fetch by helper address
        let fetched = fetch_by_helper_address(&mut db, &address).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0], cow_amm);

        // Test batch upsert
        let cow_amm2 = CowAmm {
            address: ByteArray([43u8; 20]),
            helper_address: address,
            block_number: 1,
            tradeable_tokens: vec![ByteArray([3u8; 20])],
        };
        upsert_batched(&mut db, std::slice::from_ref(&cow_amm2))
            .await
            .unwrap();

        let fetched = fetch_by_helper_address(&mut db, &address).await.unwrap();
        assert_eq!(fetched.len(), 2);

        // Test delete by addresses
        delete_by_blocks(&mut db, &[1]).await.unwrap();
        let fetched = fetch_by_helper_address(&mut db, &address).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0], cow_amm2);
    }
}
