use {
    once_cell::sync::OnceCell,
    prometheus::Encoder,
    std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc},
    tokio::task::{self, JoinHandle},
    warp::{Filter, Rejection, Reply},
};

/// Global metrics registry used by all components.
static REGISTRY: OnceCell<prometheus_metric_storage::StorageRegistry> = OnceCell::new();

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

pub fn serve_metrics(liveness: Arc<dyn LivenessChecking>, address: SocketAddr) -> JoinHandle<()> {
    let filter = handle_metrics().or(handle_liveness(liveness));
    tracing::info!(%address, "serving metrics");
    task::spawn(warp::serve(filter).bind(address))
}

// `/metrics` route exposing encoded prometheus data to monitoring system
pub fn handle_metrics() -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    let registry = get_registry();
    warp::path("metrics").map(move || encode(registry))
}

fn handle_liveness(
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
