use super::Postgres;
use anyhow::Result;
use chrono::{Duration, Utc};
use ethcontract::U256;
use futures::{StreamExt, TryStreamExt};
use model::{
    order::{Order, OrderUid},
    time::now_in_epoch_seconds,
};
use number_conversions::u256_to_big_decimal;
use shared::db_order_conversions::full_order_into_model_order;

/// New fee data to update the order with.
pub struct FeeUpdate {
    /// The actual fee amount to charge the order from its surplus.
    pub surplus_fee: U256,

    /// The full unsubsidized fee amount that settling the order is expected to
    /// actually cost. This is used for objective value computation so that fee
    /// subsidies do not change the objective value.
    pub full_fee_amount: U256,
}

impl Postgres {
    pub async fn limit_orders_with_outdated_fees(&self, age: Duration) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["limit_orders_with_outdated_fees"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        let timestamp = Utc::now() - age;
        database::orders::limit_orders_with_outdated_fees(
            &mut ex,
            timestamp,
            now_in_epoch_seconds().into(),
        )
        .map(|result| match result {
            Ok(order) => full_order_into_model_order(order),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await
    }

    pub async fn update_limit_order_fees(
        &self,
        order_uid: &OrderUid,
        update: &FeeUpdate,
    ) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_limit_order_fees"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::orders::update_limit_order_fees(
            &mut ex,
            &database::byte_array::ByteArray(order_uid.0),
            &database::orders::FeeUpdate {
                surplus_fee: u256_to_big_decimal(&update.surplus_fee),
                surplus_fee_timestamp: Utc::now(),
                full_fee_amount: u256_to_big_decimal(&update.full_fee_amount),
            },
        )
        .await?;
        Ok(())
    }
}
