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

impl Postgres {
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
