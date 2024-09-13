use {anyhow::Context, primitive_types::H256};

impl super::Postgres {
    pub async fn find_settlement_transactions(&self, auction_id: i64) -> anyhow::Result<Vec<H256>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["find_settlement_transactions"])
            .start_timer();

        let mut ex = self.pool.acquire().await.context("acquire")?;
        let hashes = database::settlements::get_hashes_by_auction_id(&mut ex, auction_id)
            .await
            .context("get_hashes_by_auction_id")?
            .into_iter()
            .map(|hash| H256(hash.0))
            .collect();
        Ok(hashes)
    }
}
