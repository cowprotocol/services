use anyhow::Context;
use primitive_types::H256;

impl super::Postgres {
    pub async fn recent_settlement_tx_hashes(&self, start_block: i64) -> anyhow::Result<Vec<H256>> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["recent_settlement_tx_hashes"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        let hashes = database::settlements::recent_settlement_tx_hashes(&mut ex, start_block)
            .await
            .context("recent_settlement_tx_hashes")?;
        Ok(hashes.into_iter().map(|hash| H256(hash.0)).collect())
    }
}
