use super::Postgres;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use database::orders::{Quote, SurplusFeeQuoteParameters};
use ethcontract::U256;
use futures::{StreamExt, TryStreamExt};
use model::time::now_in_epoch_seconds;
use number_conversions::u256_to_big_decimal;
use shared::fee_subsidy::FeeParameters;

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
    pub async fn order_parameters_with_outdated_fees(
        &self,
        age: Duration,
        limit: usize,
    ) -> Result<Vec<SurplusFeeQuoteParameters>> {
        let limit: i64 = limit.try_into().context("convert limit")?;

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["limit_orders_with_outdated_fees"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        let timestamp = Utc::now() - age;
        database::orders::order_parameters_with_most_outdated_fees(
            &mut ex,
            timestamp,
            now_in_epoch_seconds().into(),
            limit,
        )
        .map(|result| result.map_err(anyhow::Error::from))
        .try_collect()
        .await
    }

    /// Updates the `surplus_fee` of all limit orders matching the [`SurplusFeeQuoteParameters`]
    /// and stores a quote for each one.
    pub async fn update_limit_order_fees(
        &self,
        parameters: &SurplusFeeQuoteParameters,
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
                    sell_token: parameters.sell_token,
                    buy_token: parameters.buy_token,
                    sell_amount: parameters.sell_amount.clone(),
                },
                Some(database::orders::Quote {
                    // for every order we update we copy this struct and set the order_uid later
                    order_uid: Default::default(),
                    gas_amount: quote.fee_parameters.gas_amount,
                    gas_price: quote.fee_parameters.gas_price,
                    sell_token_price: quote.fee_parameters.sell_token_price,
                    sell_amount: u256_to_big_decimal(&quote.sell_amount),
                    buy_amount: u256_to_big_decimal(&quote.buy_amount),
                }),
            ),
            FeeUpdate::Failure { timestamp } => (
                // Note that the surplus fee must be removed so that the order does not count as
                // solvable. In order to be solvable the timestamp must be recent and the fee must
                // be set. We don't reset the timestamp because it indicates the last update time
                // (regardless of error or success). This is needed so that we can query the least
                // recently updated limit orders. See #965 .
                //
                // Note that we'll do a bulk update of orders so technically it's possible that an
                // error during the price estimation invalidates a multiple orders. But errors are
                // very rare and it's not very common to have identical orders anyway so we don't
                // have to protect against bulk invalidations.
                database::orders::FeeUpdate {
                    surplus_fee: None,
                    surplus_fee_timestamp: *timestamp,
                    full_fee_amount: 0.into(),
                    sell_token: parameters.sell_token,
                    buy_token: parameters.buy_token,
                    sell_amount: parameters.sell_amount.clone(),
                },
                None,
            ),
        };

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_limit_order_fees"])
            .start_timer();
        let mut ex = self.0.begin().await?;
        let updated_order_uids = database::orders::update_limit_order_fees(&mut ex, &update)
            .await
            .context("update_limit_order_fee")?;
        if let Some(quote) = quote {
            for order_uid in updated_order_uids {
                let quote = Quote {
                    order_uid,
                    ..quote.clone()
                };
                database::orders::insert_quote_and_update_on_conflict(&mut ex, &quote)
                    .await
                    .context("insert_quote_and_update_on_conflict")?;
            }
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
