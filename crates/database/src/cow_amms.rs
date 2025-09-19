use {
    crate::{Address, PgTransaction},
    sqlx::{Executor, PgConnection, QueryBuilder, Row},
    tracing::instrument,
};

/// Represents a CoW AMM stored in the database
#[derive(Debug, Clone, PartialEq)]
pub struct CowAmm {
    pub address: Address,
    pub helper_contract_address: Address,
    pub tradeable_tokens: Vec<Address>,
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

// @todo: add deployment block number column
/// Insert or update a batch of CoW AMMs in the database
#[instrument(skip_all)]
async fn upsert(ex: &mut PgConnection, cow_amms: &[CowAmm]) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        "INSERT INTO cow_amms (address, helper_contract_address, tradeable_tokens) ";
    const CONFLICT_CLAUSE: &str = r#"
ON CONFLICT (address)
DO UPDATE SET
    helper_contract_address = EXCLUDED.helper_contract_address,
    tradeable_tokens = EXCLUDED.tradeable_tokens"#;

    let mut query_builder = QueryBuilder::new(QUERY);

    query_builder.push_values(cow_amms, |mut builder, cow_amm| {
        builder
            .push_bind(cow_amm.address)
            .push_bind(cow_amm.helper_contract_address)
            .push_bind(cow_amm.tradeable_tokens.clone());
    });
    query_builder.push(CONFLICT_CLAUSE);
    query_builder.build().execute(ex).await?;

    Ok(())
}

/// Fetch all CoW AMMs for a specific helper contract
#[instrument(skip_all)]
pub async fn fetch_by_helper(
    ex: &mut PgConnection,
    helper_contract_address: &Address,
) -> Result<Vec<CowAmm>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT address, helper_contract_address, tradeable_tokens
FROM cow_amms
WHERE helper_contract_address = $1;
    "#;

    let rows = sqlx::query(QUERY)
        .bind(helper_contract_address)
        .fetch_all(ex)
        .await?;

    Ok(rows
        .into_iter()
        .map(|row| CowAmm {
            address: row.get("address"),
            helper_contract_address: row.get("helper_contract_address"),
            tradeable_tokens: row.get("tradeable_tokens"),
        })
        .collect())
}

/// Delete CoW AMMs by their addresses
#[instrument(skip_all)]
pub async fn delete_by_addresses(
    ex: &mut PgConnection,
    addresses: &[Address],
) -> Result<(), sqlx::Error> {
    if addresses.is_empty() {
        return Ok(());
    }

    const QUERY: &str = r#"
DELETE FROM cow_amms
WHERE address = ANY($1);
    "#;

    ex.execute(sqlx::query(QUERY).bind(addresses)).await?;
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

        let helper_address = ByteArray([1u8; 20]);
        let cow_amm = CowAmm {
            address: ByteArray([42u8; 20]),
            helper_contract_address: helper_address,
            tradeable_tokens: vec![ByteArray([1u8; 20]), ByteArray([2u8; 20])],
        };

        // Test upsert
        upsert(&mut db, std::slice::from_ref(&cow_amm))
            .await
            .unwrap();

        // Test fetch by helper
        let fetched = fetch_by_helper(&mut db, &helper_address).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0], cow_amm);

        // Test batch upsert
        let cow_amm2 = CowAmm {
            address: ByteArray([43u8; 20]),
            helper_contract_address: helper_address,
            tradeable_tokens: vec![ByteArray([3u8; 20])],
        };
        upsert_batched(&mut db, std::slice::from_ref(&cow_amm2))
            .await
            .unwrap();

        let fetched = fetch_by_helper(&mut db, &helper_address).await.unwrap();
        assert_eq!(fetched.len(), 2);

        // Test delete by addresses
        delete_by_addresses(&mut db, &[cow_amm.address])
            .await
            .unwrap();
        let fetched = fetch_by_helper(&mut db, &helper_address).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0], cow_amm2);
    }
}
