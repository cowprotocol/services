pub mod arguments;
pub mod database;

use crate::database::Postgres;
use shared::metrics::LivenessChecking;
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
    tokio::select! {
        result = serve_metrics => tracing::error!(?result, "serve_metrics exited"),
        _ = db_metrics => unreachable!(),
    };
}
