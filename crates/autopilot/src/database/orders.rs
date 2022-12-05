use super::Postgres;
use anyhow::Result;
use chrono::{Duration, Utc};
use database::byte_array::ByteArray;
use futures::{StreamExt, TryStreamExt};
use model::{
    order::{Order, OrderUid},
    time::now_in_epoch_seconds,
};
use number_conversions::u256_to_big_decimal;
use shared::{
    db_order_conversions::{full_order_into_model_order, order_kind_into},
    order_quoting::Quote,
};

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

    pub async fn update_limit_order_fees(&self, order_uid: &OrderUid, quote: &Quote) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_limit_order_fees"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::orders::update_limit_order_fees(
            &mut ex,
            &database::byte_array::ByteArray(order_uid.0),
            &database::orders::FeeUpdate {
                surplus_fee: u256_to_big_decimal(&quote.fee_amount),
                surplus_fee_timestamp: Utc::now(),
                full_fee_amount: u256_to_big_decimal(&quote.full_fee_amount),
            },
        )
        .await?;

        database::quotes::save(
            &mut ex,
            &database::quotes::Quote {
                id: Default::default(),
                sell_token: ByteArray(quote.data.sell_token.0),
                buy_token: ByteArray(quote.data.buy_token.0),
                sell_amount: u256_to_big_decimal(&quote.data.quoted_sell_amount),
                buy_amount: u256_to_big_decimal(&quote.data.quoted_buy_amount),
                gas_amount: quote.data.fee_parameters.gas_amount,
                gas_price: quote.data.fee_parameters.gas_price,
                sell_token_price: quote.data.fee_parameters.sell_token_price,
                order_kind: order_kind_into(quote.data.kind),
                expiration_timestamp: quote.data.expiration,
                quote_kind: quote.data.quote_kind.clone(),
            },
        )
        .await?;
        Ok(())
    }

    pub async fn count_limit_orders(&self) -> Result<i64> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["count_limit_orders"])
            .start_timer();
        let mut ex = self.0.acquire().await?;
        Ok(database::orders::count_limit_orders(&mut ex, now_in_epoch_seconds().into()).await?)
    }

    pub async fn count_limit_orders_with_outdated_fees(&self, age: Duration) -> Result<i64> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["count_limit_orders_with_outdated_fees"])
            .start_timer();
        let mut ex = self.0.acquire().await?;
        let timestamp = Utc::now() - age;
        Ok(database::orders::count_limit_orders_with_outdated_fees(
            &mut ex,
            timestamp,
            now_in_epoch_seconds().into(),
        )
        .await?)
    }
}
