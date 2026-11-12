use {
    num::ToPrimitive,
    shared::arguments::DB_MAX_CONNECTIONS_DEFAULT,
    sqlx::{Executor, PgConnection, PgPool, postgres::PgPoolOptions},
    std::{
        num::{NonZeroU32, NonZeroUsize},
        time::Duration,
    },
    tracing::instrument,
};

mod auction;
pub mod ethflow_events;
pub mod events;
pub mod fee_policies;
pub mod onchain_order_events;
pub mod order_events;
mod quotes;

pub const INSERT_BATCH_SIZE_DEFAULT: NonZeroUsize = NonZeroUsize::new(500).unwrap();

#[derive(Debug, Clone)]
pub struct Config {
    pub insert_batch_size: NonZeroUsize,
    pub max_pool_size: NonZeroU32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            insert_batch_size: INSERT_BATCH_SIZE_DEFAULT,
            max_pool_size: DB_MAX_CONNECTIONS_DEFAULT,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Postgres {
    pub pool: PgPool,
    pub config: Config,
}

impl Postgres {
    pub async fn new(url: &str, config: Config) -> sqlx::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_pool_size.get())
            .connect(url)
            .await?;

        Self::start_db_metrics_job(pool.clone());

        Ok(Self { pool, config })
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
        Self::new("postgresql://", Default::default()).await
    }

    #[instrument(skip_all)]
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

    /// Cleans the pending lists of all GIN indexes on `competition_auctions`.
    /// An index left out here gets flushed inline by whichever insert pushes
    /// its pending list past `gin_pending_list_limit`.
    pub async fn gin_clean_pending_list(&self) -> std::result::Result<(), sqlx::Error> {
        let _timer = Metrics::get()
            .database_queries
            .with_label_values(&["gin_clean_pending_competition_auctions"])
            .start_timer();
        let mut ex = self.pool.acquire().await?;
        ex.execute(
            "SELECT gin_clean_pending_list('competition_auctions_order_uids_gin'), \
             gin_clean_pending_list('competition_auctions_price_tokens_gin')",
        )
        .await
        .map(|_| ())
    }
}

async fn count_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    let query = format!("SELECT COUNT(*) FROM {table};");
    sqlx::query_scalar(&query).fetch_one(ex).await
}

/// Estimates the row count the way the query planner does: tuples per page
/// from the last `VACUUM` or `ANALYZE`, times the table's current page count.
/// Returns 0 for a table that was never vacuumed or analyzed.
async fn estimate_rows_in_table(ex: &mut PgConnection, table: &str) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT CASE WHEN relpages > 0 AND reltuples >= 0 THEN reltuples::float8 / relpages * \
         (pg_relation_size(oid) / current_setting('block_size')::int) ELSE 0 END::bigint FROM \
         pg_class WHERE oid = $1::regclass",
    )
    .bind(table)
    .fetch_one(ex)
    .await
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Number of rows in db tables.
    #[metric(labels("table"))]
    table_rows: prometheus::IntGaugeVec,

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
    // Spawn the task for database metrics
    tokio::task::spawn(database_metrics(db));
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        database::{byte_array::ByteArray, orders::Order},
    };

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

    #[tokio::test]
    #[ignore]
    async fn postgres_estimate_rows_in_table_() {
        let db = Postgres::with_defaults().await.unwrap();
        let mut ex = db.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();

        let count = estimate_rows_in_table(&mut ex, "orders").await.unwrap();
        assert_eq!(count, 0);

        for i in 0..100 {
            let order = Order {
                uid: ByteArray([i; 56]),
                ..Default::default()
            };
            database::orders::insert_order(&mut ex, &order)
                .await
                .unwrap();
        }
        sqlx::query("ANALYZE orders")
            .execute(&mut *ex)
            .await
            .unwrap();
        let analyzed = estimate_rows_in_table(&mut ex, "orders").await.unwrap();
        assert_eq!(analyzed, 100);

        for i in 100..200 {
            let order = Order {
                uid: ByteArray([i; 56]),
                ..Default::default()
            };
            database::orders::insert_order(&mut ex, &order)
                .await
                .unwrap();
        }
        let grown = estimate_rows_in_table(&mut ex, "orders").await.unwrap();
        assert!(
            grown > analyzed,
            "estimate {grown} did not grow past {analyzed}"
        );
    }
}
