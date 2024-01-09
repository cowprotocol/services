use {
    super::Postgres,
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    model::order::OrderUid,
    primitive_types::U256,
    shared::db_order_conversions::full_order_into_model_order,
    sqlx::PgConnection,
};

impl Postgres {
    /// Returns the unsubsidised fees for the given orders.
    /// For limit orders, the order fee is None.
    pub async fn order_fees(
        ex: &mut PgConnection,
        order_uids: &[OrderUid],
    ) -> Result<Vec<Option<U256>>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["order_fees"])
            .start_timer();

        let mut orders: Vec<Option<U256>> = Default::default();
        for order_uid in order_uids {
            let order = database::orders::single_full_order(ex, &ByteArray(order_uid.0))
                .await?
                .map(full_order_into_model_order)
                .context("order not found")??;

            let order_fee = if order.metadata.solver_fee == U256::zero() {
                None
            } else {
                Some(order.metadata.solver_fee)
            };
            orders.push(order_fee);
        }

        Ok(orders)
    }
}
