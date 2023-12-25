use {
    super::Postgres,
    crate::decoded_settlement::OrderExecution,
    anyhow::{Context, Result},
    database::{byte_array::ByteArray, OrderUid},
    ethcontract::H256,
    futures::{TryFutureExt, TryStreamExt},
    model::{auction::AuctionId, order::Order},
    shared::db_order_conversions::full_order_into_model_order,
    sqlx::PgConnection,
    std::collections::HashMap,
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
                .try_collect::<Vec<_>>()
                .map_err(anyhow::Error::from)
                .await?;

        let mut orders: HashMap<OrderUid, Order> = Default::default();
        for execution in executions {
            if let std::collections::hash_map::Entry::Vacant(e) = orders.entry(execution.order_uid)
            {
                let order = database::orders::single_full_order(ex, &execution.order_uid)
                    .await?
                    .map(full_order_into_model_order)
                    .context("order not found")??;

                e.insert(order);
            }

            let order = orders.get(&execution.order_uid).expect("order not found");
            order_executions.push(OrderExecution::new(order, execution));
        }

        Ok(order_executions)
    }
}
