use sqlx::{Executor, PgConnection};

pub async fn insert_or_update_last_block(
    ex: &mut PgConnection,
    index: &str,
    last_processed_block: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO last_processed_blocks (index, last_block)
VALUES ($1, $2)
ON CONFLICT (index)
DO UPDATE SET last_block = EXCLUDED.last_block;
    "#;

    ex.execute(sqlx::query(QUERY).bind(index).bind(last_processed_block))
        .await?;
    Ok(())
}

pub async fn last_block(ex: &mut PgConnection, index: &str) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number
FROM last_processed_blocks
WHERE index = $1;
    "#;

    sqlx::query_scalar(QUERY)
        .bind(index)
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_last_processed_block_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        assert_eq!(last_block(&mut db, "test").await.unwrap(), None);

        insert_or_update_last_block(&mut db, "test", 42).await.unwrap();
        assert_eq!(last_block(&mut db, "test").await.unwrap(), Some(42));

        insert_or_update_last_block(&mut db, "test", 43).await.unwrap();
        assert_eq!(last_block(&mut db, "test").await.unwrap(), Some(43));
    }
}


