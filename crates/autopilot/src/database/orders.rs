use {
    super::Postgres,
    crate::decoded_settlement::OrderExecution,
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    ethcontract::H256,
    futures::{StreamExt, TryStreamExt},
    model::auction::AuctionId,
    shared::db_order_conversions::full_order_into_model_order,
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

        let mut order_executions = Vec::new();

        let executions =
            database::orders::order_executions_in_tx(ex, &ByteArray(tx_hash.0), auction_id)
                .map(|result| result.map_err(anyhow::Error::from))
                .try_collect::<Vec<_>>()
                .await?;

        if let Some(execution) = executions.first() {
            let order = database::orders::single_full_order(ex, &execution.order_uid)
                .await?
                .map(full_order_into_model_order)
                .context("order not found")??;

            for execution in executions {
                order_executions.push(OrderExecution::new(&order, execution)?);
            }
        }

        Ok(order_executions)
    }
}
