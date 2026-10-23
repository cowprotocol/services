pub mod app_data;
pub mod auctions;
pub mod debug_report;
mod fee_policies;
pub mod orders;
pub mod quotes;
pub mod solver_competition_v2;
pub mod total_surplus;
pub mod trades;

use {
    crate::database::orders::InsertionError,
    anyhow::Result,
    database::byte_array::ByteArray,
    model::order::Order,
    shared::arguments::DB_MAX_CONNECTIONS_DEFAULT,
    sqlx::{
        Connection,
        PgConnection,
        PgPool,
        postgres::{PgConnectOptions, PgPoolOptions},
    },
    std::time::Duration,
};

// TODO: There is remaining optimization potential by implementing sqlx encoding
// and decoding for U256 directly instead of going through BigDecimal. This is
// not very important as this is fast enough anyway.

#[derive(Debug, Clone)]
pub struct Config {
    pub max_pool_size: u32,
    /// Maps directly to Postgres' `statement_timeout` parameter, applied on a
    /// per-connection basis, but *only* if the pool was created using
    /// [`Postgres::try_new_with_timeout`].
    pub statement_timeout: Duration,
    /// Connections returned to the pool that still have pending work after
    /// this long get closed instead of reused, which makes the server abort
    /// that work.
    pub connection_release_timeout: Duration,
    /// Maps directly to Postgres' `client_connection_check_interval`
    /// parameter. Without these checks the server only notices a closed
    /// connection when it sends the results, so closing a connection would not
    /// stop its query.
    pub client_connection_check_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Match SQLx default pool size
            max_pool_size: DB_MAX_CONNECTIONS_DEFAULT.get(),
            statement_timeout: Duration::from_secs(30),
            connection_release_timeout: Duration::from_secs(1),
            client_connection_check_interval: Duration::from_secs(1),
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
            pool: pool_options(&config).connect_lazy_with(connect_options(uri, &config)?),
            config,
        })
    }

    /// Creates a Postgres connection pool, but applies the [`Config`]'s
    /// `statement_timeout` to all queries.
    pub fn try_new_with_timeout(uri: &str, config: Config) -> Result<Self> {
        let options = connect_options(uri, &config)?.options([(
            "statement_timeout",
            format!("{}ms", config.statement_timeout.as_millis()),
        )]);
        Ok(Self {
            pool: pool_options(&config).connect_lazy_with(options),
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

fn connect_options(uri: &str, config: &Config) -> Result<PgConnectOptions> {
    Ok(uri.parse::<PgConnectOptions>()?.options([(
        "client_connection_check_interval",
        format!("{}ms", config.client_connection_check_interval.as_millis()),
    )]))
}

fn pool_options(config: &Config) -> PgPoolOptions {
    let release_timeout = config.connection_release_timeout;
    PgPoolOptions::new()
        .max_connections(config.max_pool_size)
        .after_release(move |conn, _meta| {
            Box::pin(async move {
                // Dropping a query's future (e.g. because the HTTP request got
                // cancelled) does not stop the query on the server and sqlx
                // waits for it to finish before reusing the connection. That
                // blocks a pool slot for as long as the query runs. The server
                // answers a ping only after all pending work is done, so a
                // timeout means the connection is still busy. Closing it frees
                // the slot and lets the server abort the query.
                match tokio::time::timeout(release_timeout, conn.ping()).await {
                    Ok(result) => result.map(|()| true),
                    Err(_) => {
                        Metrics::get().busy_connections_closed.inc();
                        Ok(false)
                    }
                }
            })
        })
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Timing of db queries.
    #[metric(name = "orderbook_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,

    /// Connections that got closed because they were still busy when they
    /// were returned to the pool.
    #[metric(name = "orderbook_database_busy_connections_closed")]
    busy_connections_closed: prometheus::IntCounter,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::Executor, std::time::Instant};

    #[tokio::test]
    #[ignore]
    async fn postgres_statement_timeout_cancels_slow_query() {
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

    async fn backend_pid(conn: &mut PgConnection) -> i32 {
        sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(conn)
            .await
            .unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_busy_connection_gets_closed_on_release() {
        let config = Config {
            max_pool_size: 1,
            connection_release_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let db = Postgres::try_new("postgresql://", config).unwrap();

        let mut conn = db.pool.acquire().await.unwrap();
        let pid = backend_pid(&mut conn).await;
        // Simulates a request that gets cancelled while its query is running.
        let abandoned = async move { conn.execute("SELECT pg_sleep(30)").await };
        tokio::time::timeout(Duration::from_millis(100), abandoned)
            .await
            .expect_err("query should still be running");

        // The pool has a single connection, so this only succeeds in time if
        // the busy connection got closed instead of waiting for its query.
        let mut conn = tokio::time::timeout(Duration::from_secs(5), db.pool.acquire())
            .await
            .expect("busy connection should have been closed")
            .unwrap();
        assert_ne!(backend_pid(&mut conn).await, pid);

        let deadline = Instant::now() + Duration::from_secs(5);
        while sqlx::query_scalar("SELECT EXISTS (SELECT FROM pg_stat_activity WHERE pid = $1)")
            .bind(pid)
            .fetch_one(&mut *conn)
            .await
            .unwrap()
        {
            assert!(
                Instant::now() < deadline,
                "server should have aborted the abandoned query"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_idle_connection_gets_reused_on_release() {
        let config = Config {
            max_pool_size: 1,
            ..Default::default()
        };
        let db = Postgres::try_new("postgresql://", config).unwrap();

        let mut conn = db.pool.acquire().await.unwrap();
        let pid = backend_pid(&mut conn).await;
        drop(conn);

        // Rolling back an abandoned transaction is quick, so the connection
        // should be reused as well.
        let mut tx = db.pool.begin().await.unwrap();
        assert_eq!(backend_pid(&mut tx).await, pid);
        drop(tx);

        let mut conn = db.pool.acquire().await.unwrap();
        assert_eq!(backend_pid(&mut conn).await, pid);
    }
}
