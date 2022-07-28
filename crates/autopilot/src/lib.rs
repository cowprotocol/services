pub mod arguments;
pub mod database;

use crate::database::Postgres;
use shared::{metrics::LivenessChecking, transport::http::HttpTransport, Web3Transport};
use std::sync::Arc;

struct Liveness;
#[async_trait::async_trait]
impl LivenessChecking for Liveness {
    async fn is_alive(&self) -> bool {
        true
    }
}

/// Assumes tracing and metrics registry have already been set up.
pub async fn main(args: arguments::Arguments) {
    let serve_metrics = shared::metrics::serve_metrics(Arc::new(Liveness), args.metrics_address);
    let db = Postgres::new(args.db_url.as_str()).await.unwrap();
    let db_metrics = crate::database::database_metrics(db);
    let client = shared::http_client(args.http_timeout);
    let transport = Web3Transport::new(HttpTransport::new(
        client.clone(),
        args.node_url.clone(),
        "".to_string(),
    ));
    let web3 = web3::Web3::new(transport);
    let network_id = web3.net().version().await.unwrap();
    tracing::info!("network_id {network_id}");
    tokio::select! {
        result = serve_metrics => tracing::error!(?result, "serve_metrics exited"),
        _ = db_metrics => unreachable!(),
    };
}
