use prometheus::{Encoder, Registry};
use std::net::SocketAddr;
use tokio::task::{self, JoinHandle};
use warp::{Filter, Rejection, Reply};

pub const DEFAULT_METRICS_PORT: u16 = 9586;

pub fn serve_metrics(registry: Registry, address: SocketAddr) -> JoinHandle<()> {
    let filter = handle_metrics(registry);
    tracing::info!(%address, "serving metrics");
    task::spawn(warp::serve(filter).bind(address))
}

// `/metrics` route exposing encoded prometheus data to monitoring system
pub fn handle_metrics(
    registry: Registry,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
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
