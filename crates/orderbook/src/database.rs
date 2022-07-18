pub mod events;
pub mod orders;
pub mod quotes;
pub mod solver_competition;
pub mod trades;

use anyhow::Result;
use sqlx::{Executor, PgPool, Row};

// TODO: There is remaining optimization potential by implementing sqlx encoding and decoding for
// U256 directly instead of going through BigDecimal. This is not very important as this is fast
// enough anyway.

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Postgres {
    pub pool: PgPool,
}

// The implementation is split up into several modules which contain more public methods.

impl Postgres {
    pub fn new(uri: &str) -> Result<Self> {
        Ok(Self {
            pool: PgPool::connect_lazy(uri)?,
        })
    }

    async fn count_rows_in_table(&self, table: &str) -> Result<i64> {
        let query = format!("SELECT COUNT(*) FROM {};", table);
        let row = self.pool.fetch_one(query.as_str()).await?;
        row.try_get(0).map_err(Into::into)
    }

    pub async fn update_table_rows_metric(&self) -> Result<()> {
        let metrics = Metrics::get();
        for &table in database::ALL_TABLES {
            let count = self.count_rows_in_table(table).await?;
            metrics.table_rows.with_label_values(&[table]).set(count);
        }
        Ok(())
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Number of rows in db tables.
    #[metric(labels("table"))]
    table_rows: prometheus::IntGaugeVec,

    /// Timing of db queries.
    #[metric(labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::orders::OrderStoring;

    #[tokio::test]
    #[ignore]
    async fn postgres_count_rows_in_tables_works() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        let count = db.count_rows_in_table("orders").await.unwrap();
        assert_eq!(count, 0);

        db.insert_order(&Default::default(), Default::default())
            .await
            .unwrap();
        let count = db.count_rows_in_table("orders").await.unwrap();
        assert_eq!(count, 1);
    }
}
