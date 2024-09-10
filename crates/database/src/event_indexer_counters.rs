use sqlx::{Executor, PgConnection};

pub async fn insert_or_update_counter(
    ex: &mut PgConnection,
    name: &str,
    value: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO event_indexer_counters (name, counter)
VALUES ($1, $2)
ON CONFLICT (name)
DO UPDATE SET counter = EXCLUDED.counter;
    "#;

    ex.execute(sqlx::query(QUERY).bind(name).bind(value))
        .await?;
    Ok(())
}

pub async fn current_value(ex: &mut PgConnection, name: &str) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT counter
FROM event_indexer_counters
WHERE name = $1;
    "#;

    sqlx::query_scalar(QUERY)
        .bind(name)
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Connection};

    #[tokio::test]
    #[ignore]
    async fn postgres_event_indexer_counter_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        assert_eq!(current_value(&mut db, "test").await.unwrap(), None);

        insert_or_update_counter(&mut db, "test", 42).await.unwrap();
        assert_eq!(current_value(&mut db, "test").await.unwrap(), Some(42));

        insert_or_update_counter(&mut db, "test", 43).await.unwrap();
        assert_eq!(current_value(&mut db, "test").await.unwrap(), Some(43));
    }
}
