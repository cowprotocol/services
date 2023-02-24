use {
    prometheus::Encoder,
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
    let registry = global_metrics::get_metrics_registry();
    warp::path("metrics").map(move || {
        let encoder = prometheus::TextEncoder::new();
        let mut buffer = Vec::new();
        if let Err(e) = encoder.encode(&registry.gather(), &mut buffer) {
            tracing::error!("could not encode metrics: {}", e);
        };
        match String::from_utf8(buffer) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("metrics could not be from_utf8'd: {}", e);
                String::default()
            }
        }
    })
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
