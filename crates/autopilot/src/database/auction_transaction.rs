use database::{auction_transaction::SettlementEvent, byte_array::ByteArray};
use model::auction::AuctionId;
use primitive_types::{H160, H256};

impl super::Postgres {
    pub async fn upsert_auction_transaction(
        &self,
        auction_id: AuctionId,
        tx_from: &H160,
        tx_nonce: i64,
    ) -> Result<(), sqlx::Error> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["upsert_auction_transaction"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::auction_transaction::upsert_auction_transaction(
            &mut ex,
            auction_id,
            &ByteArray(tx_from.0),
            tx_nonce,
        )
        .await
    }

    pub async fn insert_settlement_tx_info(
        &self,
        block_number: i64,
        log_index: i64,
        tx_from: &H160,
        tx_nonce: i64,
    ) -> Result<(), sqlx::Error> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["insert_settlement_tx_info"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::auction_transaction::insert_settlement_tx_info(
            &mut ex,
            block_number,
            log_index,
            &ByteArray(tx_from.0),
            tx_nonce,
        )
        .await?;
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

    pub async fn get_auction_id_from_tx_hash(
        &self,
        tx_hash: &H256,
    ) -> Result<Option<i64>, sqlx::Error> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["get_auction_id_from_tx_hash"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        database::auction_transaction::get_auction_id_from_tx_hash(&mut ex, &ByteArray(tx_hash.0))
            .await
    }
}
