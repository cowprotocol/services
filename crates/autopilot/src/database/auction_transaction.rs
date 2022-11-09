use anyhow::Context;
use database::{auction_transaction::SettlementEvent, byte_array::ByteArray};
use futures::FutureExt;
use primitive_types::{H160, H256};
use sqlx::Connection;

impl super::Postgres {
    pub async fn update_settlement_tx_info(
        &self,
        block_number: i64,
        log_index: i64,
        tx_from: H160,
        tx_nonce: i64,
        tx_hash: H256,
    ) -> anyhow::Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["update_settlement_tx_info"])
            .start_timer();

        let mut ex = self.0.acquire().await?;
        // This all happens in one transaction to prevent the situation where we would update
        // settlements without updating auction_transaction.
        ex.transaction(|ex| {
            async move {
                database::auction_transaction::insert_settlement_tx_info(
                    ex,
                    block_number,
                    log_index,
                    &ByteArray(tx_from.0),
                    tx_nonce,
                )
                .await
                .context("insert_settlement_tx_info")?;

                // Auctions that were stored before the auction_transaction table existed need to be
                // inserted into it.
                // This code can be removed when all old auctions have been processed this way.
                let auction_id = match database::auction_transaction::get_auction_id_from_tx_hash(
                    ex,
                    &ByteArray(tx_hash.0),
                )
                .await
                .context("get_auction_id_from_tx_hash")?
                {
                    Some(auction_id) => auction_id,
                    None => return Ok(()),
                };
                tracing::trace!(auction_id);

                // If this is an auction that was stored after the auction_transaction table existed then
                // the row has already been inserted. This should not be treated as an error.
                // Inserting exactly the same row again as it already exist does not error in Postgres. If
                // this wasn't the case we would need to check the the error reason is the primary key
                // constraint to find out whether the error is benign.
                // Because this insert does not error there is no need for the check. If our insert changed
                // any column we would correctly error.
                database::auction_transaction::upsert_auction_transaction(
                    ex,
                    auction_id,
                    &ByteArray(tx_from.0),
                    tx_nonce,
                )
                .await
                .context("upsert_auction_transaction")?;

                Ok(())
            }
            .boxed()
        })
        .await
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
