use {anyhow::Context, primitive_types::H256};

impl super::Postgres {
    pub async fn find_tx_hash_by_auction_id(
        &self,
        auction_id: i64,
    ) -> anyhow::Result<Option<H256>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["find_tx_hash_by_auction_id"])
            .start_timer();

        let mut ex = self.pool.acquire().await.context("acquire")?;
        let hash = database::settlements::get_hash_by_auction_id(&mut ex, auction_id)
            .await
            .context("get_hash_by_auction_id")?;
        Ok(hash.map(|hash| H256(hash.0)))
    }
}
