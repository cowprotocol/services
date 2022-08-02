pub mod arguments;
pub mod database;
pub mod event_updater;

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
    let db_metrics = crate::database::database_metrics(db.clone());
    let client = shared::http_client(args.http_timeout);
    let transport = Web3Transport::new(HttpTransport::new(
        client.clone(),
        args.node_url.clone(),
        "".to_string(),
    ));
    let web3 = web3::Web3::new(transport);
    let network_id = web3.net().version().await.unwrap();
    tracing::info!("network_id {network_id}");
    let current_block_stream = shared::current_block::current_block_stream(
        web3.clone(),
        args.block_stream_poll_interval_seconds,
    )
    .await
    .unwrap();
    let settlement_contract = contracts::GPv2Settlement::deployed(&web3)
        .await
        .expect("Couldn't load deployed settlement");

    let sync_start = if args.skip_event_sync {
        web3.eth()
            .block_number()
            .await
            .map(|block| block.as_u64())
            .ok()
    } else {
        None
    };
    let event_updater = Arc::new(event_updater::EventUpdater::new(
        settlement_contract.clone(),
        db.clone(),
        sync_start,
    ));

    let service_maintainer = shared::maintenance::ServiceMaintenance {
        maintainers: vec![event_updater],
    };
    let maintenance_task =
        tokio::task::spawn(service_maintainer.run_maintenance_on_new_block(current_block_stream));

    tokio::select! {
        result = serve_metrics => tracing::error!(?result, "serve_metrics exited"),
        _ = db_metrics => unreachable!(),
        _ = maintenance_task => unreachable!(),
    };
}
