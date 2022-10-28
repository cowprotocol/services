pub mod arguments;
pub mod refund_service;

use contracts::CoWSwapEthFlow;
use refund_service::RefundService;
use shared::http_client::HttpClientFactory;
use sqlx::PgPool;
use std::time::Duration;

const SLEEP_TIME_BETWEEN_LOOPS: u64 = 30;

pub async fn main(args: arguments::Arguments) {
    let pg_pool = PgPool::connect_lazy(args.db_url.as_str()).expect("failed to create database");
    let http_factory = HttpClientFactory::new(&args.http_client);
    let web3 = shared::web3(&http_factory, &args.node_url, "base");
    let ethflow_contract = CoWSwapEthFlow::deployed(&web3)
        .await
        .expect("Could not find the CoWSwapEthFlow contract");
    let refunder = RefundService::new(
        pg_pool,
        web3,
        ethflow_contract,
        args.min_validity_duration.as_secs() as i64,
        args.min_slippage_bps,
    );
    loop {
        tracing::info!("Staring a new refunding loop");
        match refunder.try_to_refund_all_eligble_orders().await {
            Ok(_) => (),
            Err(err) => tracing::error!("Error while refunding ethflow orders: {:?}", err),
        }
        tokio::time::sleep(Duration::from_secs(SLEEP_TIME_BETWEEN_LOOPS)).await;
    }
}
