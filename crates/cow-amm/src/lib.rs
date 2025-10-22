mod amm;
mod cache;
mod factory;
mod maintainers;
mod registry;

pub use {amm::Amm, contracts::CowAmmLegacyHelper as Helper, registry::Registry};

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
