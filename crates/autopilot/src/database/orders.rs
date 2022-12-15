use super::Postgres;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use database::byte_array::ByteArray;
use ethcontract::U256;
use futures::{StreamExt, TryStreamExt};
use model::{
    order::{Order, OrderUid},
    time::now_in_epoch_seconds,
};
use number_conversions::u256_to_big_decimal;
use shared::{db_order_conversions::full_order_into_model_order, fee_subsidy::FeeParameters};

/// New fee data to update a limit order with.
///
/// Both success and failure to calculate the new fee are recorded in the database.
pub enum FeeUpdate {
    Success {
        timestamp: DateTime<Utc>,
        /// The actual fee amount to charge the order from its surplus.
        surplus_fee: U256,
        /// The full unsubsidized fee amount that settling the order is expected to
        /// actually cost. This is used for objective value computation so that fee
        /// subsidies do not change the objective value.
        full_fee_amount: U256,
        quote: LimitOrderQuote,
    },
    Failure {
        timestamp: DateTime<Utc>,
    },
}

/// Data required to compute risk adjusted rewards for limit orders.
pub struct LimitOrderQuote {
    /// Everything required to compute the fee amount in sell token
    pub fee_parameters: FeeParameters,

    /// The `sell_amount` of the quote associated with the `surplus_fee` estimation.
    pub sell_amount: U256,

    /// The `buy_amount` of the quote associated with the `surplus_fee` estimation.
    pub buy_amount: U256,
}

impl Postgres {
    pub async fn limit_orders_with_outdated_fees(&self, age: Duration) -> Result<Vec<Order>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["limit_orders_with_outdated_fees"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        let timestamp = Utc::now() - age;
        database::orders::limit_orders_with_most_outdated_fees(
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

    /// Updates the `surplus_fee` of a limit order together with the quote used to compute that
    /// fee.
    pub async fn update_limit_order_fees(
        &self,
        order_uid: &OrderUid,
        update: &FeeUpdate,
    ) -> Result<()> {
        let (update, quote) = match update {
            FeeUpdate::Success {
                timestamp,
                surplus_fee,
                full_fee_amount,
                quote,
            } => (
                database::orders::FeeUpdate {
                    surplus_fee: Some(u256_to_big_decimal(surplus_fee)),
                    surplus_fee_timestamp: *timestamp,
                    full_fee_amount: u256_to_big_decimal(full_fee_amount),
                },
                Some(database::orders::Quote {
                    order_uid: ByteArray(order_uid.0),
                    gas_amount: quote.fee_parameters.gas_amount,
                    gas_price: quote.fee_parameters.gas_price,
                    sell_token_price: quote.fee_parameters.sell_token_price,
                    sell_amount: u256_to_big_decimal(&quote.sell_amount),
                    buy_amount: u256_to_big_decimal(&quote.buy_amount),
                }),
            ),
            FeeUpdate::Failure { timestamp } => (
                database::orders::FeeUpdate {
                    surplus_fee: None,
                    surplus_fee_timestamp: *timestamp,
                    full_fee_amount: 0.into(),
                },
                None,
            ),
        };

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_limit_order_fees"])
            .start_timer();
        let mut ex = self.0.begin().await?;
        database::orders::update_limit_order_fees(&mut ex, &ByteArray(order_uid.0), &update)
            .await
            .context("update_limit_order_fee")?;
        if let Some(quote) = quote {
            database::orders::insert_quote_and_update_on_conflict(&mut ex, &quote)
                .await
                .context("insert_quote_and_update_on_conflict")?;
        }
        ex.commit().await.context("commit")?;
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
