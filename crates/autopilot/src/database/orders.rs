use {
    super::Postgres,
    crate::decoded_settlement::OrderExecution,
    anyhow::Result,
    database::byte_array::ByteArray,
    ethcontract::H256,
    futures::{StreamExt, TryStreamExt},
    model::auction::AuctionId,
    sqlx::PgConnection,
};

/// New fee data to update a limit order with.
///
/// Both success and failure to calculate the new fee are recorded in the
/// database.
// pub enum FeeUpdate {
//     Success {
//         timestamp: DateTime<Utc>,
//         /// The actual fee amount to charge the order from its surplus.
//         surplus_fee: U256,
//         /// The full unsubsidized fee amount that settling the order is expected
//         /// to actually cost. This is used for objective value
//         /// computation so that fee subsidies do not change the
//         /// objective value.
//         full_fee_amount: U256,
//         quote: LimitOrderQuote,
//     },
//     Failure {
//         timestamp: DateTime<Utc>,
//     },
// }

/// Data required to compute risk adjusted rewards for limit orders.
// pub struct LimitOrderQuote {
//     /// Everything required to compute the fee amount in sell token
//     pub fee_parameters: FeeParameters,

//     /// The `sell_amount` of the quote associated with the `surplus_fee`
//     /// estimation.
//     pub sell_amount: U256,

//     /// The `buy_amount` of the quote associated with the `surplus_fee`
//     /// estimation.
//     pub buy_amount: U256,

//     /// The solver that provided the quote.
//     pub solver: H160,
// }

impl Postgres {
    /// Returns all limit orders that are waiting to be filled.
    // pub async fn open_fok_limit_orders(&self, age: Duration) ->
    // Result<Vec<OrderQuotingData>> {     let _timer = super::Metrics::get()
    //         .database_queries
    //         .with_label_values(&["open_limit_orders"])
    //         .start_timer();

    //     let mut ex = self.0.acquire().await?;
    //     let timestamp = Utc::now() - age;
    //     database::orders::open_fok_limit_orders(&mut ex, timestamp,
    // now_in_epoch_seconds().into())         .map(|result|
    // result.map_err(anyhow::Error::from))         .try_collect()
    //         .await
    // }

    // pub async fn count_fok_limit_orders(&self) -> Result<i64> {
    //     let _timer = super::Metrics::get()
    //         .database_queries
    //         .with_label_values(&["count_limit_orders"])
    //         .start_timer();
    //     let mut ex = self.0.acquire().await?;
    //     Ok(
    //         database::orders::count_fok_limit_orders(&mut ex,
    // now_in_epoch_seconds().into())             .await?,
    //     )
    // }

    pub async fn order_executions_for_tx(
        ex: &mut PgConnection,
        tx_hash: &H256,
        auction_id: AuctionId,
    ) -> Result<Vec<OrderExecution>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["orders_for_tx"])
            .start_timer();

        database::orders::order_executions_in_tx(ex, &ByteArray(tx_hash.0), auction_id)
            .map(|result| match result {
                Ok(execution) => execution.try_into().map_err(Into::into),
                Err(err) => Err(anyhow::Error::from(err)),
            })
            .try_collect()
            .await
    }
}
