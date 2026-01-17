use {
    num::ToPrimitive,
    sqlx::{Executor, PgConnection, PgPool},
    std::{num::NonZeroUsize, time::Duration},
    tracing::Instrument,
};

mod auction;
pub mod auction_prices;
pub mod competition;
pub mod ethflow_events;
pub mod events;
pub mod fee_policies;
pub mod onchain_order_events;
pub mod order_events;
mod quotes;

#[derive(Debug, Clone)]
pub struct Config {
    pub insert_batch_size: NonZeroUsize,
}

#[derive(Debug, Clone)]
pub struct Postgres {
    pub pool: PgPool,
    pub config: Config,
}

impl Postgres {
    pub async fn new(url: &str, insert_batch_size: NonZeroUsize) -> sqlx::Result<Self> {
        let pool = PgPool::connect(url).await?;

        Self::start_db_metrics_job(pool.clone());

        Ok(Self {
            pool,
            config: Config { insert_batch_size },
        })
    }

    fn start_db_metrics_job(pool: PgPool) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(5));
            loop {
                ticker.tick().await;

                let Some(idle) = pool.num_idle().to_i64() else {
                    tracing::error!("Failed to get number of idle connections from the pool");
                    continue;
                };
                let active = i64::from(pool.size()) - idle;

                Metrics::get().active_connections.set(active);
                Metrics::get().idle_connections.set(idle);
            }
        });
    }

    pub async fn with_defaults() -> sqlx::Result<Self> {
        Self::new("postgresql://", NonZeroUsize::new(500).unwrap()).await
    }

    pub async fn update_database_metrics(&self) -> sqlx::Result<()> {
        let metrics = Metrics::get();

        let mut ex = self.pool.acquire().await?;

        // update table row metrics
        for &table in database::TABLES {
            let count = count_rows_in_table(&mut ex, table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }

        // update table row metrics
        for &table in database::LARGE_TABLES {
            let count = estimate_rows_in_table(&mut ex, table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }

        Ok(())
    }

    pub async fn update_large_tables_stats(&self) -> sqlx::Result<()> {
        let mut ex = self.pool.acquire().await?;
        for &table in database::LARGE_TABLES {
            analyze_table(&mut ex, table).await?;
        }

        Ok(())
    }
}

async fn count_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table};");
    sqlx::query_scalar(&query).fetch_one(ex).await
}

async fn estimate_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    let query = format!("SELECT reltuples::bigint FROM pg_class WHERE relname='{table}';");
    sqlx::query_scalar(&query).fetch_one(ex).await
}

async fn analyze_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<()> {
    let query = format!("ANALYZE {table};");
    ex.execute(sqlx::query(&query)).await.map(|_| ())
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
    #[metric(
        name = "autopilot_database_queries",
        labels("type"),
        buckets(
            0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0,
            60.0, 70.0, 80.0, 90.0
        )
    )]
    database_queries: prometheus::HistogramVec,

    /// Number of active connections in the database pool.
    #[metric(name = "database_active_connections")]
    active_connections: prometheus::IntGauge,

    /// Number of idle connections in the database pool.
    #[metric(name = "database_idle_connections")]
    idle_connections: prometheus::IntGauge,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

pub fn run_database_metrics_work(db: Postgres) {
    let span = tracing::info_span!("database_metrics");
    // Spawn the task for updating large table statistics
    tokio::spawn(update_large_tables_stats(db.clone()).instrument(span.clone()));
    // Spawn the task for database metrics
    tokio::task::spawn(database_metrics(db).instrument(span));
}

async fn database_metrics(db: Postgres) -> ! {
    loop {
        // The DB gets used a lot right after starting the system.
        // Since these queries are quite expensive we delay them
        // to improve the startup time of the system.
        tokio::time::sleep(Duration::from_secs(60)).await;
        if let Err(err) = db.update_database_metrics().await {
            tracing::error!(?err, "failed to update table rows metric");
        }
    }
}

async fn update_large_tables_stats(db: Postgres) -> ! {
    loop {
        if let Err(err) = db.update_large_tables_stats().await {
            tracing::error!(?err, "failed to update large tables stats");
        }
        tokio::time::sleep(Duration::from_secs(60 * 60)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_count_rows_in_table_() {
        let db = Postgres::with_defaults().await.unwrap();
        let mut ex = db.pool.begin().await.unwrap();
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
