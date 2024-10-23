use sqlx::{Executor, PgConnection};

pub async fn update(
    ex: &mut PgConnection,
    contract: &str,
    last_indexed_block: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO last_indexed_blocks (contract, block_number)
VALUES ($1, $2)
ON CONFLICT (contract)
DO UPDATE SET block_number = EXCLUDED.block_number;
    "#;

    ex.execute(sqlx::query(QUERY).bind(contract).bind(last_indexed_block))
        .await?;
    Ok(())
}

pub async fn fetch(ex: &mut PgConnection, contract: &str) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number
FROM last_indexed_blocks
WHERE contract = $1;
    "#;

    sqlx::query_scalar(QUERY)
        .bind(contract)
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_last_indexed_block_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        assert_eq!(fetch(&mut db, "test").await.unwrap(), None);

        update(&mut db, "test", 42).await.unwrap();
        assert_eq!(fetch(&mut db, "test").await.unwrap(), Some(42));

        update(&mut db, "test", 43).await.unwrap();
        assert_eq!(fetch(&mut db, "test").await.unwrap(), Some(43));
    }
}
