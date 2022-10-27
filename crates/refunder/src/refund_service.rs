use anyhow::Result;
use database::ethflow_orders::refundable_orders;
use sqlx::PgPool;
use std::time::SystemTime;

pub struct RefundService {
    pub db: PgPool,
    pub min_validity_duration: i64,
    pub min_slippage: f64,
}

impl RefundService {
    pub fn new(db: PgPool, min_validity_duration: i64, min_slippage: f64) -> Self {
        RefundService {
            db,
            min_validity_duration,
            min_slippage,
        }
    }

    pub async fn try_to_refund_all_eligble_orders(&self) -> Result<()> {
        // Step 1:
        // Look for all refundable orders by calling the database
        let unix_timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut ex = self.db.acquire().await?;
        let _refundable_order_uids = refundable_orders(
            &mut ex,
            unix_timestamp,
            self.min_validity_duration,
            self.min_slippage,
        )
        .await?;

        // Step 2:
        // take out orderUids that are already refunded
        // by making a batched web3 call. Update refunded ones.

        // Step 3:
        // Send out tx to deleteOrders, and wait for 5 block for the tx
        // to be mined

        Ok(())
    }
}
