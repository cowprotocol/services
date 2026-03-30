pub mod app_data;
pub mod auction_prices;
pub mod auctions;
pub mod debug_report;
mod fee_policies;
pub mod orders;
pub mod quotes;
pub mod solver_competition;
pub mod solver_competition_v2;
pub mod total_surplus;
pub mod trades;

use {
    crate::database::orders::InsertionError,
    anyhow::Result,
    database::byte_array::ByteArray,
    model::order::Order,
    shared::arguments::DB_MAX_CONNECTIONS_DEFAULT,
    sqlx::{Executor, PgConnection, PgPool, postgres::PgPoolOptions},
    std::time::Duration,
};

// TODO: There is remaining optimization potential by implementing sqlx encoding
// and decoding for U256 directly instead of going through BigDecimal. This is
// not very important as this is fast enough anyway.

#[derive(Debug, Clone)]
pub struct Config {
    pub max_pool_size: u32,
    /// Maps directly to Postgres' `statement_timeout` parameter, applied on a
    /// per-connectionb basis, but *only* if the pool was created using
    /// [`Postgres::try_new_with_timeout`].
    pub statement_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Match SQLx default pool size
            max_pool_size: DB_MAX_CONNECTIONS_DEFAULT.get(),
            statement_timeout: Duration::from_secs(30),
        }
    }
}

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Postgres {
    pub pool: PgPool,
    pub config: Config,
}

// The implementation is split up into several modules which contain more public
// methods.

impl Postgres {
    pub fn try_new(uri: &str, config: Config) -> Result<Self> {
        Ok(Self {
            pool: PgPoolOptions::new()
                .max_connections(config.max_pool_size)
                .connect_lazy(uri)?,
            config,
        })
    }

    /// Creates a Postgres connection pool, but applies the [`Config`]'s
    /// `read_query_timeout` to all queries.
    pub fn try_new_with_timeout(uri: &str, config: Config) -> Result<Self> {
        let read_query_timeout = config.statement_timeout;

        Ok(Self {
            pool: PgPoolOptions::new()
                .max_connections(config.max_pool_size)
                .after_connect(move |conn, _meta| {
                    Box::pin(async move {
                        let timeout_ms = read_query_timeout.as_millis();
                        conn.execute(format!("SET statement_timeout = {timeout_ms}").as_str())
                            .await?;
                        Ok(())
                    })
                })
                .connect_lazy(uri)?,
            config,
        })
    }

    async fn insert_order_app_data(
        order: &Order,
        ex: &mut PgConnection,
    ) -> Result<(), InsertionError> {
        if let Some(full_app_data) = order.metadata.full_app_data.as_ref() {
            let contract_app_data = &ByteArray(order.data.app_data.0);
            let full_app_data = full_app_data.as_bytes();
            if let Some(existing) =
                database::app_data::insert(ex, contract_app_data, full_app_data).await?
                && full_app_data != existing
            {
                return Err(InsertionError::AppDataMismatch(existing));
            }
        }
        Ok(())
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Timing of db queries.
    #[metric(name = "orderbook_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Executor};

    #[tokio::test]
    #[ignore]
    async fn statement_timeout_cancels_slow_query() {
        let config = Config {
            statement_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let db = Postgres::try_new_with_timeout("postgresql://", config).unwrap();
        let mut conn = db.pool.acquire().await.unwrap();

        // A fast query should succeed.
        conn.execute("SELECT 1").await.unwrap();

        // A query exceeding the timeout should fail.
        let err = conn
            .execute("SELECT pg_sleep(5)")
            .await
            .expect_err("should have timed out");
        let db_err = err.as_database_error().expect("should be a database error");
        assert_eq!(db_err.code().as_deref(), Some("57014")); // query_canceled
    }
}
