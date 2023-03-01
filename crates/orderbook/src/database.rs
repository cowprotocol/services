pub mod auctions;
pub mod orders;
pub mod quotes;
pub mod solver_competition;
pub mod trades;

use {anyhow::Result, sqlx::PgPool};

// TODO: There is remaining optimization potential by implementing sqlx encoding
// and decoding for U256 directly instead of going through BigDecimal. This is
// not very important as this is fast enough anyway.

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Postgres {
    pub pool: PgPool,
}

// The implementation is split up into several modules which contain more public
// methods.

impl Postgres {
    pub fn new(uri: &str) -> Result<Self> {
        Ok(Self {
            pool: PgPool::connect_lazy(uri)?,
        })
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
        Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap()
    }
}
