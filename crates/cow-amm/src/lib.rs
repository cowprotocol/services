mod amm;
mod cache;
mod factory;
mod maintainers;
mod registry;
pub mod signing;

pub use {
    amm::Amm,
    contracts::alloy::cow_amm::CowAmmLegacyHelper::Instance as Helper,
    registry::Registry,
};

#[derive(prometheus_metric_storage::MetricStorage)]
pub(crate) struct Metrics {
    /// How log db queries take.
    #[metric(name = "cow_amm_database_queries", labels("type"))]
    database_queries: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}
