mod auction;
pub mod auction_prices;
pub mod auction_transaction;
pub mod competition;
pub mod ethflow_events;
mod events;
pub mod on_settlement_event_updater;
pub mod onchain_order_events;
pub mod order_events;
pub mod orders;
mod quotes;
pub mod recent_settlements;

use {
    sqlx::{PgConnection, PgPool},
    std::time::Duration,
};

#[derive(Debug, Clone)]
pub struct Postgres(pub PgPool);

impl Postgres {
    pub async fn new(url: &str) -> sqlx::Result<Self> {
        Ok(Self(PgPool::connect(url).await?))
    }

    pub async fn update_database_metrics(&self) -> sqlx::Result<()> {
        let metrics = Metrics::get();

        // update table row metrics
        for &table in database::TABLES {
            let mut ex = self.0.acquire().await?;
            let count = count_rows_in_table(&mut ex, table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }

        // update table row metrics
        for &table in database::LARGE_TABLES {
            let mut ex = self.0.acquire().await?;
            let count = estimate_rows_in_table(&mut ex, table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }

        // update unused app data metric
        {
            let mut ex = self.0.acquire().await?;
            let count = count_unused_app_data(&mut ex).await?;
            metrics.unused_app_data.set(count);
        }

        Ok(())
    }
}

async fn count_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table};");
    sqlx::query_scalar(&query).fetch_one(ex).await
}

async fn estimate_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    let query = format!("SELECT reltuples FROM pg_class WHERE relname='{table}';");
    sqlx::query_scalar(&query).fetch_one(ex).await
}

async fn count_unused_app_data(ex: &mut PgConnection) -> sqlx::Result<i64> {
    let query = r#"
        SELECT
            COUNT(*)
        FROM app_data AS a
        LEFT JOIN orders o
            ON a.contract_app_data = o.app_data
        WHERE
            o.app_data IS NULL
        ;
    "#;
    sqlx::query_scalar(query).fetch_one(ex).await
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Number of rows in db tables.
    #[metric(labels("table"))]
    table_rows: prometheus::IntGaugeVec,

    /// Number of unused app data entries.
    ///
    /// These are entries in the `app_data` table that do not have a
    /// corresponding order in the `orders` table.
    unused_app_data: prometheus::IntGauge,

    /// Timing of db queries.
    #[metric(name = "autopilot_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

pub async fn database_metrics(db: Postgres) -> ! {
    loop {
        if let Err(err) = db.update_database_metrics().await {
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
