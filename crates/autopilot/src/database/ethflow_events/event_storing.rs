//! Implements the logic for indexing `OrderRefund` events of the ethflow
//! contract.

use {
    crate::database::{events::bytes_to_order_uid, Postgres},
    anyhow::Result,
    database::ethflow_orders::Refund,
    ethrpc::block_stream::RangeInclusive,
    shared::event_handling::{EventStoring, PgEventCounter},
    sqlx::PgPool,
};

fn get_refunds(events: Vec<ethcontract::Event<EthFlowEvent>>) -> Result<Vec<Refund>> {
    events
        .into_iter()
        .filter_map(|event| {
            let (tx_hash, block_number) = match event.meta {
                Some(meta) => (meta.transaction_hash, meta.block_number),
                None => return Some(Err(anyhow::anyhow!("event without metadata"))),
            };
            let order_uid = match event.data {
                EthFlowEvent::OrderRefund(event) => event.order_uid,
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

type EthFlowEvent = contracts::cowswap_eth_flow::Event;

#[async_trait::async_trait]
impl EventStoring<EthFlowEvent> for Postgres {
    async fn append_events(&mut self, events: Vec<ethcontract::Event<EthFlowEvent>>) -> Result<()> {
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
        events: Vec<ethcontract::Event<EthFlowEvent>>,
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

    async fn last_event_block(&self) -> Result<u64> {
        PgEventCounter::last_event_block(self).await
    }

    async fn update_counter(&mut self, new_value: u64) -> Result<()> {
        PgEventCounter::update_counter(self, new_value).await
    }
}

#[async_trait::async_trait]
impl PgEventCounter<EthFlowEvent> for Postgres {
    const INDEXER_NAME: &'static str = "ethflow_refund_indexer";

    fn pg_pool(&self) -> &PgPool {
        &self.pool
    }
}
