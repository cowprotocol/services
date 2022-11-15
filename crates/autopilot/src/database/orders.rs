use super::Postgres;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use ethcontract::U256;
use futures::{StreamExt, TryStreamExt};
use model::{order::Order, time::now_in_epoch_seconds};
use number_conversions::u256_to_big_decimal;
use shared::{db_order_conversions::full_order_into_model_order, limit_order_quoter};

#[async_trait]
impl limit_order_quoter::Storing for Postgres {
    async fn update_surplus_fee(&self, order: &Order, surplus_fee: U256) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_surplus_fee"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::orders::update_surplus_fee(
            &mut ex,
            &database::byte_array::ByteArray(order.metadata.uid.0),
            &u256_to_big_decimal(&surplus_fee),
            Utc::now(),
        )
        .await?;
        Ok(())
    }

    async fn limit_orders_with_outdated_fees(&self, age: Duration) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["limit_orders_with_outdated_fees"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::orders::limit_orders_with_outdated_fees(
            &mut ex,
            age,
            now_in_epoch_seconds().into(),
        )
        .map(|result| match result {
            Ok(order) => full_order_into_model_order(order),
            Err(err) => Err(anyhow::Error::from(err)),
        })
        .try_collect()
        .await
    }
}
