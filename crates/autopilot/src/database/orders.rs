use {
    anyhow::Context,
    database::byte_array::ByteArray,
    model::order::OrderUid,
    sqlx::PgConnection,
    std::collections::HashSet,
};

impl super::Postgres {
    pub async fn get_missing_order_uids(
        order_uids: impl Iterator<Item = OrderUid>,
        ex: &mut PgConnection,
    ) -> anyhow::Result<HashSet<OrderUid>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_missing_order_uids"])
            .start_timer();
        let missing_uids = database::orders::get_missing_order_uids(
            ex,
            order_uids.map(|uid| ByteArray(uid.0)).collect(),
        )
        .await
        .context("get_missing_order_uids".to_string())?;

        Ok(missing_uids
            .into_iter()
            .map(|uid| OrderUid(uid.0))
            .collect::<HashSet<_>>())
    }
}
