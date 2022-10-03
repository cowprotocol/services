mod auction;
mod events;
pub mod onchain_order_events;
mod quotes;

use sqlx::{PgConnection, PgPool};
use std::time::Duration;

#[derive(Clone)]
pub struct Postgres(pub PgPool);

impl Postgres {
    pub async fn new(url: &str) -> sqlx::Result<Self> {
        Ok(Self(PgPool::connect(url).await?))
    }

    pub async fn update_table_rows_metric(&self) -> sqlx::Result<()> {
        let metrics = Metrics::get();
        for &table in database::ALL_TABLES {
            let mut ex = self.0.acquire().await?;
            let count = count_rows_in_table(&mut ex, table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }
        Ok(())
    }
}

async fn count_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {};", table);
    sqlx::query_scalar(&query).fetch_one(ex).await
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Number of rows in db tables.
    #[metric(labels("table"))]
    table_rows: prometheus::IntGaugeVec,

    /// Timing of db queries.
    #[metric(name = "autopilot_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap()
    }
}

pub async fn database_metrics(db: Postgres) -> ! {
    loop {
        if let Err(err) = db.update_table_rows_metric().await {
            tracing::error!(?err, "failed to update table rows metric");
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_count_rows_in_table_() {
        let db = Postgres::new("postgresql://").await.unwrap();
        let mut ex = db.0.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();

        let count = count_rows_in_table(&mut ex, "orders").await.unwrap();
        assert_eq!(count, 0);
        database::orders::insert_order(&mut ex, &Default::default())
            .await
            .unwrap();
        let count = count_rows_in_table(&mut ex, "orders").await.unwrap();
        assert_eq!(count, 1);
    }
}
