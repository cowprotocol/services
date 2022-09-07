use super::Postgres;
use anyhow::{anyhow, Context, Result};
use contracts::gpv2_settlement::{
    event_data::{
        OrderInvalidated as ContractInvalidation, PreSignature as ContractPreSignature,
        Settlement as ContractSettlement, Trade as ContractTrade,
    },
    Event as ContractEvent,
};
use database::{
    byte_array::ByteArray,
    events::{Event, EventIndex, Invalidation, PreSignature, Settlement, Trade},
    OrderUid,
};
use ethcontract::{Event as EthContractEvent, EventMetadata};
use number_conversions::u256_to_big_decimal;
use shared::event_handling::EventStoring;
use std::convert::TryInto;

pub fn contract_to_db_events(
    contract_events: Vec<EthContractEvent<ContractEvent>>,
) -> Result<Vec<(EventIndex, Event)>> {
    contract_events
        .into_iter()
        .filter_map(|EthContractEvent { data, meta }| {
            let meta = match meta {
                Some(meta) => meta,
                None => return Some(Err(anyhow!("event without metadata"))),
            };
            match data {
                ContractEvent::Trade(event) => Some(convert_trade(&event, &meta)),
                ContractEvent::Settlement(event) => Some(Ok(convert_settlement(&event, &meta))),
                ContractEvent::OrderInvalidated(event) => Some(convert_invalidation(&event, &meta)),
                ContractEvent::PreSignature(event) => Some(convert_presignature(&event, &meta)),
                // TODO: handle new events
                ContractEvent::Interaction(_) => None,
            }
        })
        .collect::<Result<Vec<_>>>()
}

#[async_trait::async_trait]
impl EventStoring<ContractEvent> for Postgres {
    async fn last_event_block(&self) -> Result<u64> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["last_event_block"])
            .start_timer();

        let mut con = self.0.acquire().await?;
        let block_number = database::events::last_block(&mut con)
            .await
            .context("block_number_of_most_recent_event failed")?;

        Ok(block_number
            .try_into()
            .context("block number is negative")?)
    }

    async fn append_events(&mut self, events: Vec<EthContractEvent<ContractEvent>>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["append_events"])
            .start_timer();

        let events = contract_to_db_events(events)?;
        let mut transaction = self.0.begin().await?;
        database::events::append(&mut transaction, &events)
            .await
            .context("append_events")?;
        transaction.commit().await.context("commit")?;
        Ok(())
    }

    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<ContractEvent>>,
        range: std::ops::RangeInclusive<u64>,
    ) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["replace_events"])
            .start_timer();

        let events = contract_to_db_events(events)?;
        let mut transaction = self.0.begin().await?;
        database::events::delete(&mut transaction, *range.start() as i64)
            .await
            .context("delete_events failed")?;
        database::events::append(&mut transaction, events.as_slice())
            .await
            .context("insert_events failed")?;
        transaction.commit().await.context("commit")?;
        Ok(())
    }
}

fn meta_to_event_index(meta: &EventMetadata) -> EventIndex {
    EventIndex {
        block_number: meta.block_number as i64,
        log_index: meta.log_index as i64,
    }
}

fn bytes_to_order_uid(bytes: &[u8]) -> Result<OrderUid> {
    bytes
        .try_into()
        .context("order_uid has wrong number of bytes")
        .map(ByteArray)
}

fn convert_trade(trade: &ContractTrade, meta: &EventMetadata) -> Result<(EventIndex, Event)> {
    let event = Trade {
        order_uid: bytes_to_order_uid(&trade.order_uid.0)?,
        sell_amount_including_fee: u256_to_big_decimal(&trade.sell_amount),
        buy_amount: u256_to_big_decimal(&trade.buy_amount),
        fee_amount: u256_to_big_decimal(&trade.fee_amount),
    };
    Ok((meta_to_event_index(meta), Event::Trade(event)))
}

fn convert_settlement(
    settlement: &ContractSettlement,
    meta: &EventMetadata,
) -> (EventIndex, Event) {
    let event = Settlement {
        solver: ByteArray(settlement.solver.0),
        transaction_hash: ByteArray(meta.transaction_hash.0),
    };
    (meta_to_event_index(meta), Event::Settlement(event))
}

fn convert_invalidation(
    invalidation: &ContractInvalidation,
    meta: &EventMetadata,
) -> Result<(EventIndex, Event)> {
    let event = Invalidation {
        order_uid: bytes_to_order_uid(&invalidation.order_uid.0)?,
    };
    Ok((meta_to_event_index(meta), Event::Invalidation(event)))
}

fn convert_presignature(
    presignature: &ContractPreSignature,
    meta: &EventMetadata,
) -> Result<(EventIndex, Event)> {
    let event = PreSignature {
        owner: ByteArray(presignature.owner.0),
        order_uid: ByteArray(
            presignature
                .order_uid
                .0
                .as_slice()
                .try_into()
                .context("trade event order_uid has wrong number of bytes")?,
        ),
        signed: presignature.signed,
    };
    Ok((meta_to_event_index(meta), Event::PreSignature(event)))
}
