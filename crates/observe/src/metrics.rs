use {
    prometheus::{
        Encoder,
        core::{AtomicF64, AtomicU64, GenericCounterVec},
    },
    std::{
        collections::HashMap,
        convert::Infallible,
        net::SocketAddr,
        sync::{
            Arc,
            OnceLock,
            atomic::{AtomicBool, Ordering},
        },
        time::Instant,
    },
    tokio::task::{self, JoinHandle},
    warp::{Filter, Rejection, Reply},
};

/// Global metrics registry used by all components.
static REGISTRY: OnceLock<prometheus_metric_storage::StorageRegistry> = OnceLock::new();

/// Configure global metrics registry.
///
/// This function allows specifying common prefix that will be added
/// to all metric names, as well as common labels.
///
/// This function can be called at most once, and it should be done before
/// any call to [`get_registry`], ideally in the very beginning
/// of the `main` function.
///
/// # Panics
///
/// This function panics if it's called twice, or if it's called after
/// any call to [`get_registry`]. This function also panics if registry
/// configuration is invalid.
pub fn setup_registry(prefix: Option<String>, labels: Option<HashMap<String, String>>) {
    let registry = prometheus::Registry::new_custom(prefix, labels).unwrap();
    let storage_registry = prometheus_metric_storage::StorageRegistry::new(registry);
    REGISTRY.set(storage_registry).unwrap();
}

/// Like [`setup_registry`], but can be called multiple times in a row.
/// Later calls are ignored.
///
/// Useful for tests.
pub fn setup_registry_reentrant(prefix: Option<String>, labels: Option<HashMap<String, String>>) {
    let registry = prometheus::Registry::new_custom(prefix, labels).unwrap();
    let storage_registry = prometheus_metric_storage::StorageRegistry::new(registry);
    REGISTRY.set(storage_registry).ok();
}

/// Get the global instance of the metrics registry.
pub fn get_registry() -> &'static prometheus::Registry {
    get_storage_registry().registry()
}

/// Get the global instance of the metric storage registry.
///
/// # Implementation notice
///
/// If global metrics registry was not configured with [`setup_registry`],
/// it will be initialized using a default value. We could've panic instead,
/// but panicking creates troubles for unit tests. There is no way to set up
/// a hook that will call [`setup_registry`] before each test, so we'll
/// have to initialize it manually before every test, which is tedious
/// to say the least.
pub fn get_storage_registry() -> &'static prometheus_metric_storage::StorageRegistry {
    REGISTRY.get_or_init(prometheus_metric_storage::StorageRegistry::default)
}

pub fn encode(registry: &prometheus::Registry) -> String {
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&registry.gather(), &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

pub const DEFAULT_METRICS_PORT: u16 = 9586;

#[async_trait::async_trait]
pub trait LivenessChecking: Send + Sync {
    async fn is_alive(&self) -> bool;
}

pub fn serve_metrics(
    liveness: Arc<dyn LivenessChecking>,
    address: SocketAddr,
    readiness: Arc<Option<AtomicBool>>,
    startup: Arc<Option<AtomicBool>>,
) -> JoinHandle<()> {
    let filter = handle_metrics()
        .or(handle_liveness_probe(liveness))
        .or(handle_readiness_probe(readiness))
        .or(handle_startup_probe(startup));
    tracing::info!(%address, "serving metrics");
    task::spawn(warp::serve(filter).bind(address))
}

// `/metrics` route exposing encoded prometheus data to monitoring system
pub fn handle_metrics() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let registry = get_registry();
    warp::path("metrics").map(move || encode(registry))
}

// Axum version of `/metrics` route
#[cfg(feature = "axum-tracing")]
pub fn handle_metrics_axum() -> axum::Router {
    async fn metrics_handler() -> String {
        encode(get_registry())
    }

    axum::Router::new().route("/metrics", axum::routing::get(metrics_handler))
}

fn handle_liveness_probe(
    liveness_checker: Arc<dyn LivenessChecking>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path("liveness").and_then(move || {
        let liveness_checker = liveness_checker.clone();
        async move {
            let status = if liveness_checker.is_alive().await {
                warp::http::StatusCode::OK
            } else {
                warp::http::StatusCode::SERVICE_UNAVAILABLE
            };
            Result::<_, Infallible>::Ok(warp::reply::with_status(warp::reply(), status))
        }
    })
}

fn handle_readiness_probe(
    readiness: Arc<Option<AtomicBool>>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path("ready").and_then(move || {
        let readiness = readiness.clone();
        async move {
            let Some(ref readiness) = *readiness else {
                // if readiness is not configured we're ready right away
                return Result::<_, Infallible>::Ok(warp::reply::with_status(
                    warp::reply(),
                    warp::http::StatusCode::OK,
                ));
            };
            let status = if readiness.load(Ordering::Acquire) {
                warp::http::StatusCode::OK
            } else {
                warp::http::StatusCode::SERVICE_UNAVAILABLE
            };
            Result::<_, Infallible>::Ok(warp::reply::with_status(warp::reply(), status))
        }
    })
}

fn handle_startup_probe(
    startup: Arc<Option<AtomicBool>>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path("startup").and_then(move || {
        let startup = startup.clone();
        async move {
            let Some(ref startup) = *startup else {
                // if startup is not configured we're started right away
                return Result::<_, Infallible>::Ok(warp::reply::with_status(
                    warp::reply(),
                    warp::http::StatusCode::OK,
                ));
            };
            let status = if startup.load(Ordering::Acquire) {
                warp::http::StatusCode::OK
            } else {
                warp::http::StatusCode::SERVICE_UNAVAILABLE
            };
            Result::<_, Infallible>::Ok(warp::reply::with_status(warp::reply(), status))
        }
    })
}

/// Metrics shared by potentially all processes.
#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// All the time losses we incur while arbitrating the auctions
    #[metric(labels("component", "phase"))]
    pub auction_overhead_time: GenericCounterVec<AtomicF64>,

    /// How many measurements we did for each source of overhead.
    #[metric(labels("component", "phase"))]
    pub auction_overhead_count: GenericCounterVec<AtomicU64>,
}

impl Metrics {
    /// Returns a struct that measures the overhead when it gets dropped.
    #[must_use]
    pub fn on_auction_overhead_start<'a, 'b, 'c>(
        &'a self,
        component: &'b str,
        phase: &'c str,
    ) -> impl Drop + use<'a, 'b, 'c> {
        let start = std::time::Instant::now();
        scopeguard::guard(start, move |start| {
            self.measure_auction_overhead(start, component, phase);
        })
    }

    pub fn measure_auction_overhead(&self, start: Instant, component: &str, phase: &str) {
        self.auction_overhead_time
            .with_label_values(&[component, phase])
            .inc_by(start.elapsed().as_secs_f64());

        self.auction_overhead_count
            .with_label_values(&[component, phase])
            .inc()
    }
}

pub fn metrics() -> &'static Metrics {
    Metrics::instance(get_storage_registry()).unwrap()
}
