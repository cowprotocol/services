use sqlx::{types::JsonValue, PgConnection};

pub async fn save(ex: &mut PgConnection, data: &JsonValue) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO registered_pools (json)
VALUES ($1)"#;
    sqlx::query(QUERY).bind(data).execute(ex).await?;
    Ok(())
}
