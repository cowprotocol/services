pub mod app_data;
pub mod auctions;
mod fee_policies;
pub mod orders;
pub mod quotes;
pub mod solver_competition;
pub mod total_surplus;
pub mod trades;

use {
    crate::database::orders::InsertionError,
    anyhow::Result,
    database::byte_array::ByteArray,
    model::order::Order,
    sqlx::{PgConnection, PgPool},
};

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

    async fn insert_order_app_data(
        order: &Order,
        ex: &mut PgConnection,
    ) -> Result<(), InsertionError> {
        if let Some(full_app_data) = order.metadata.full_app_data.as_ref() {
            let contract_app_data = &ByteArray(order.data.app_data.0);
            let full_app_data = full_app_data.as_bytes();
            if let Some(existing) =
                database::app_data::insert(ex, contract_app_data, full_app_data).await?
            {
                if full_app_data != existing {
                    return Err(InsertionError::AppDataMismatch(existing));
                }
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
