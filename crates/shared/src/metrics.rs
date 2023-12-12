use {
    std::{convert::Infallible, net::SocketAddr, sync::Arc},
    tokio::task::{self, JoinHandle},
    warp::{Filter, Rejection, Reply},
};

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
    let registry = observe::metrics::get_registry();
    warp::path("metrics").map(move || observe::metrics::encode(registry))
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
