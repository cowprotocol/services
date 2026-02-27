//! Implements the logic for indexing `OrderRefund` events of the ethflow
//! contract.
use {
    crate::database::{Postgres, events::bytes_to_order_uid},
    alloy::rpc::types::Log,
    anyhow::Result,
    contracts::CoWSwapEthFlow::CoWSwapEthFlow::CoWSwapEthFlowEvents,
    database::ethflow_orders::Refund,
    ethrpc::block_stream::RangeInclusive,
    shared::event_handling::EventStoring,
};

fn get_refunds(events: Vec<(CoWSwapEthFlowEvents, Log)>) -> Result<Vec<Refund>> {
    events
        .into_iter()
        .filter_map(|(event, log)| {
            let Log {
                transaction_hash: Some(tx_hash),
                block_number: Some(block_number),
                ..
            } = log
            else {
                return Some(Err(anyhow::anyhow!("event without metadata")));
            };

            let order_uid = match event {
                CoWSwapEthFlowEvents::OrderRefund(event) => event.orderUid,
                _ => return None,
            };
            let order_uid = match bytes_to_order_uid(&order_uid.0) {
                Ok(uid) => uid,
                Err(err) => return Some(Err(err)),
            };
            Some(Ok(Refund {
                order_uid,
                tx_hash: database::byte_array::ByteArray(tx_hash.0),
                block_number,
            }))
        })
        .collect()
}

/// This name is used to store the latest indexed block in the db.
pub(crate) const INDEX_NAME: &str = "ethflow_refunds";

#[async_trait::async_trait]
impl EventStoring<(CoWSwapEthFlowEvents, Log)> for Postgres {
    async fn last_event_block(&self) -> Result<u64> {
        crate::boundary::events::read_last_block_from_db(&self.pool, INDEX_NAME).await
    }

    async fn persist_last_indexed_block(&mut self, last_block: u64) -> Result<()> {
        crate::boundary::events::write_last_block_to_db(&self.pool, last_block, INDEX_NAME).await
    }

    async fn append_events(&mut self, events: Vec<(CoWSwapEthFlowEvents, Log)>) -> Result<()> {
        let refunds = match get_refunds(events)? {
            refunds if !refunds.is_empty() => refunds,
            _ => return Ok(()),
        };
        let _timer = crate::database::Metrics::get()
            .database_queries
            .with_label_values(&["append_ethflow_refund_events"])
            .start_timer();
        let mut ex = self.pool.begin().await?;
        database::ethflow_orders::insert_refund_tx_hashes(&mut ex, &refunds).await?;
        ex.commit().await?;
        Ok(())
    }

    async fn replace_events(
        &mut self,
        events: Vec<(CoWSwapEthFlowEvents, Log)>,
        range: RangeInclusive<u64>,
    ) -> Result<()> {
        let refunds = get_refunds(events)?;
        let _timer = crate::database::Metrics::get()
            .database_queries
            .with_label_values(&["replace_ethflow_refund_events"])
            .start_timer();
        let mut ex = self.pool.begin().await?;
        database::ethflow_orders::delete_refunds(
            &mut ex,
            i64::try_from(*range.start()).unwrap_or(i64::MAX),
            i64::try_from(*range.end()).unwrap_or(i64::MAX),
        )
        .await?;
        database::ethflow_orders::insert_refund_tx_hashes(&mut ex, &refunds).await?;
        ex.commit().await?;
        Ok(())
    }
}
