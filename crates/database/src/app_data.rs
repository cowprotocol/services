use {crate::AppId, sqlx::PgConnection};

/// Tries to associate the contract app data with the full app data.
///
/// If this contract app data already existed then the existing full app data is
/// returned, otherwise `None` is returned.
pub async fn insert(
    ex: &mut PgConnection,
    contract_app_data: &AppId,
    full_app_data: &[u8],
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    const QUERY: &str = r#"
WITH inserted AS (
    INSERT INTO app_data (contract_app_data, full_app_data)
    VALUES ($1, $2)
    -- returns null on conflict
    ON CONFLICT DO NOTHING
    -- returns TRUE if the insertion succeeded
    RETURNING TRUE
)
SELECT
    CASE
        WHEN (SELECT * FROM inserted) THEN
            NULL
        ELSE
            (SELECT full_app_data FROM app_data WHERE contract_app_data = $1)
    END
;"#;
    sqlx::query_scalar(QUERY)
        .bind(contract_app_data)
        .bind(full_app_data)
        .fetch_one(ex)
        .await
}

pub async fn fetch(
    ex: &mut PgConnection,
    contract_app_data: &AppId,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT full_app_data
FROM app_data
WHERE contract_app_data = $1
;"#;
    sqlx::query_scalar(QUERY)
        .bind(contract_app_data)
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {super::*, crate::byte_array::ByteArray, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_app_data() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut tx = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let contract = ByteArray([0u8; 32]);
        // fetch non existant app data
        let result = fetch(&mut db, &contract).await.unwrap();
        assert!(result.is_none());

        let full = vec![1u8];
        let result = insert(&mut db, &contract, &full).await.unwrap();
        assert_eq!(result, None);

        // now exists
        let result = fetch(&mut db, &contract).await.unwrap();
        assert_eq!(result, Some(full.clone()));

        // insert again with same app data
        let result = insert(&mut db, &contract, &full).await.unwrap();
        assert_eq!(result, Some(full.clone()));

        // insert again with different app data fails
        let result = insert(&mut db, &contract, &[4, 2]).await.unwrap();
        assert_eq!(result, Some(full));
    }
}
