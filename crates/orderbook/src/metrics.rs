use anyhow::Result;
use gas_estimation::EstimatedGasPrice;
use prometheus::{
    Gauge, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGaugeVec, Opts,
};
use shared::{
    metrics::get_metrics_registry,
    sources::{
        balancer_v2::pool_fetching::BalancerPoolCacheMetrics,
        uniswap_v2::pool_cache::PoolCacheMetrics,
    },
    transport::instrumented::TransportMetrics,
};
use std::time::Duration;

pub struct Metrics {
    db_table_row_count: IntGaugeVec,
    /// Outgoing RPC request metrics
    rpc_requests: HistogramVec,
    pool_cache_hits: IntCounter,
    pool_cache_misses: IntCounter,
    database_queries: HistogramVec,
    /// Gas estimate metrics
    gas_price: Gauge,
    price_estimates: IntCounterVec,
    price_estimator_cache: IntCounterVec,
}

impl Metrics {
    pub fn new() -> Result<Self> {
        let registry = get_metrics_registry();

        let db_table_row_count = IntGaugeVec::new(
            Opts::new("table_rows", "Number of rows in db tables."),
            &["table"],
        )?;
        registry.register(Box::new(db_table_row_count.clone()))?;

        let opts = HistogramOpts::new(
            "transport_requests",
            "RPC Request durations labelled by method",
        );
        let rpc_requests = HistogramVec::new(opts, &["method"]).unwrap();
        registry.register(Box::new(rpc_requests.clone()))?;

        let pool_cache_hits = IntCounter::new(
            "pool_cache_hits",
            "Number of cache hits in the pool fetcher cache.",
        )?;
        registry.register(Box::new(pool_cache_hits.clone()))?;
        let pool_cache_misses = IntCounter::new(
            "pool_cache_misses",
            "Number of cache misses in the pool fetcher cache.",
        )?;
        registry.register(Box::new(pool_cache_misses.clone()))?;

        let opts = HistogramOpts::new(
            "database_queries",
            "Sql queries to our postgresql database.",
        );
        let database_queries = HistogramVec::new(opts, &["type"]).unwrap();
        registry.register(Box::new(database_queries.clone()))?;

        let opts = Opts::new("gas_price", "Gas price estimate over time.");
        let gas_price = Gauge::with_opts(opts).unwrap();
        registry.register(Box::new(gas_price.clone()))?;

        let price_estimates = IntCounterVec::new(
            Opts::new("price_estimates", "Price estimator success/failure counter"),
            &["estimator_type", "result"],
        )?;
        registry.register(Box::new(price_estimates.clone()))?;

        let price_estimator_cache = IntCounterVec::new(
            Opts::new(
                "price_estimator_cache",
                "Price estimator cache hit/miss counter.",
            ),
            &["estimator_type", "result"],
        )?;
        registry.register(Box::new(price_estimator_cache.clone()))?;

        Ok(Self {
            db_table_row_count,
            rpc_requests,
            pool_cache_hits,
            pool_cache_misses,
            database_queries,
            gas_price,
            price_estimates,
            price_estimator_cache,
        })
    }

    pub fn set_table_row_count(&self, table: &str, count: i64) {
        self.db_table_row_count
            .with_label_values(&[table])
            .set(count);
    }
}

impl TransportMetrics for Metrics {
    fn report_query(&self, label: &str, elapsed: Duration) {
        self.rpc_requests
            .with_label_values(&[label])
            .observe(elapsed.as_secs_f64())
    }
}

impl PoolCacheMetrics for Metrics {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize) {
        self.pool_cache_hits.inc_by(cache_hits as u64);
        self.pool_cache_misses.inc_by(cache_misses as u64);
    }
}

impl crate::database::instrumented::Metrics for Metrics {
    fn database_query_histogram(&self, label: &str) -> Histogram {
        self.database_queries.with_label_values(&[label])
    }
}

impl crate::gas_price::Metrics for Metrics {
    fn gas_price(&self, estimate: EstimatedGasPrice) {
        self.gas_price.set(estimate.effective_gas_price() / 1e9);
    }
}

impl shared::price_estimation::instrumented::Metrics for Metrics {
    fn initialize_estimator(&self, name: &str) {
        for result in ["success", "failure"] {
            self.price_estimates
                .with_label_values(&[name, result])
                .reset();
        }
    }

    fn price_estimated(&self, name: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        self.price_estimates
            .with_label_values(&[name, result])
            .inc();
    }
}

impl BalancerPoolCacheMetrics for Metrics {
    fn pools_fetched(&self, cache_hits: usize, cache_misses: usize) {
        // We may want to distinguish cache metrics between the different
        // liquidity sources in the future, for now just use the same counters.
        self.pool_cache_hits.inc_by(cache_hits as u64);
        self.pool_cache_misses.inc_by(cache_misses as u64);
    }
}

impl shared::price_estimation::cached::Metrics for Metrics {
    fn price_estimator_cache(&self, name: &str, misses: usize, hits: usize) {
        self.price_estimator_cache
            .with_label_values(&[name, "misses"])
            .inc_by(misses as u64);
        self.price_estimator_cache
            .with_label_values(&[name, "hits"])
            .inc_by(hits as u64);
    }
}
