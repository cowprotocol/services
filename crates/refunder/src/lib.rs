pub mod arguments;
pub mod refund_service;

use refund_service::RefundService;
use sqlx::PgPool;
use std::time::Duration;

const SLEEP_TIME_BETWEEN_LOOPS: u64 = 30;

pub async fn main(args: arguments::Arguments) {
    let pg_pool = PgPool::connect_lazy(args.db_url.as_str())
        .expect("failed to create database");
    let refunder = RefundService::new(
        pg_pool,
        args.min_validity_duration.as_secs() as i64,
        args.min_slippage,
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
