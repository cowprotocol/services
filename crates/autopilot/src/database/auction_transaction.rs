use anyhow::Context;
use database::{auction_transaction::SettlementEvent, byte_array::ByteArray};
use primitive_types::H160;

impl super::Postgres {
    pub async fn update_settlement_tx_info(
        &self,
        block_number: i64,
        log_index: i64,
        tx_from: H160,
        tx_nonce: i64,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_tx_info"])
            .start_timer();

        let mut ex = self.0.acquire().await.context("acquire")?;
        database::auction_transaction::insert_settlement_tx_info(
            &mut ex,
            block_number,
            log_index,
            &ByteArray(tx_from.0),
            tx_nonce,
        )
        .await
        .context("insert_settlement_tx_info")?;

        Ok(())
    }

    pub async fn get_settlement_event_without_tx_info(
        &self,
        max_block_number: i64,
    ) -> Result<Option<SettlementEvent>, sqlx::Error> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_settlement_event_without_tx_info"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::auction_transaction::get_settlement_event_without_tx_info(
            &mut ex,
            max_block_number,
        )
        .await
    }
}
