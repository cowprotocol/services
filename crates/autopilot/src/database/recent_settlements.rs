use std::ops::Range;

use anyhow::Context;
use primitive_types::H256;

impl super::Postgres {
    pub async fn recent_settlement_tx_hashes(
        &self,
        block_range: Range<u64>,
    ) -> anyhow::Result<Vec<H256>> {
        let start: i64 = block_range.start.try_into()?;
        let end: i64 = block_range.end.try_into()?;
        let block_range = start..end;

        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["recent_settlement_tx_hashes"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        let hashes = database::settlements::recent_settlement_tx_hashes(&mut ex, block_range)
            .await
            .context("recent_settlement_tx_hashes")?;
        Ok(hashes.into_iter().map(|hash| H256(hash.0)).collect())
    }
}
