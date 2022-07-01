use sqlx::PgExecutor;

// The names of all tables we use in the db.
pub const ALL_TABLES: &[&str] = &[
    "orders",
    "trades",
    "invalidations",
    "quotes",
    "settlements",
    "presignature_events",
    "order_quotes",
];

/// Delete all data in the database. Only used by tests.
#[allow(non_snake_case)]
pub async fn clear_DANGER(ex: impl PgExecutor<'_> + Copy) -> sqlx::Result<()> {
    for table in ALL_TABLES {
        ex.execute(format!("TRUNCATE {};", table).as_str()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_clear() {
        let db = sqlx::PgPool::connect("postgresql://").await.unwrap();
        clear_DANGER(&db).await.unwrap();
    }
}
