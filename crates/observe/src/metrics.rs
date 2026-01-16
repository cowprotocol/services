use {
    axum::{
        Router,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
    },
    prometheus::{
        Encoder,
        core::{AtomicF64, AtomicU64, GenericCounterVec},
    },
    std::{
        collections::HashMap,
        net::SocketAddr,
        sync::{
            Arc,
            OnceLock,
            atomic::{AtomicBool, Ordering},
        },
        time::Instant,
    },
    tokio::task::JoinHandle,
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

#[derive(Clone)]
struct AppState {
    liveness: Arc<dyn LivenessChecking>,
    readiness: Arc<Option<AtomicBool>>,
    startup: Arc<Option<AtomicBool>>,
}

/// Creates a Router with only the `/metrics` endpoint.
/// Use this when you only need metrics and want to compose your own axum
/// application.
pub fn metrics_only_router() -> Router {
    Router::new().route("/metrics", get(handle_metrics))
}

/// Serves metrics on a dedicated server at the given address.
/// Spawns a background task and returns its handle.
pub fn serve_metrics(
    liveness: Arc<dyn LivenessChecking>,
    address: SocketAddr,
    readiness: Arc<Option<AtomicBool>>,
    startup: Arc<Option<AtomicBool>>,
) -> JoinHandle<()> {
    let app = Router::new()
        .route("/metrics", get(handle_metrics))
        .route("/liveness", get(handle_liveness_probe))
        .route("/ready", get(handle_readiness_probe))
        .route("/startup", get(handle_startup_probe))
        .with_state(AppState {
            liveness,
            readiness,
            startup,
        });

    tracing::info!(%address, "serving metrics");
    tokio::spawn(async move {
        axum::Server::bind(&address)
            .serve(app.into_make_service())
            .await
            .expect("failed to serve metrics")
    })
}

// `/metrics` route exposing encoded prometheus data to monitoring system
async fn handle_metrics() -> String {
    encode(get_registry())
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

async fn handle_readiness_probe(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let Some(ref readiness) = *state.readiness else {
        // if readiness is not configured we're ready right away
        return StatusCode::OK.into_response();
    };
    let status = if readiness.load(Ordering::Acquire) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    status.into_response()
}

async fn handle_startup_probe(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let Some(ref startup) = *state.startup else {
        // if startup is not configured we're started right away
        return StatusCode::OK.into_response();
    };
    let status = if startup.load(Ordering::Acquire) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    status.into_response()
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
