use {
    alloy::rpc::types::Log,
    anyhow::{Context, Result},
    contracts::alloy::GPv2Settlement::GPv2Settlement::{self, GPv2SettlementEvents},
    database::{
        OrderUid,
        PgTransaction,
        TransactionHash,
        byte_array::ByteArray,
        events::{Event, EventIndex, Invalidation, PreSignature, Settlement, Trade},
    },
    number::conversions::u256_to_big_decimal,
    std::convert::TryInto,
};

pub fn contract_to_db_events(
    contract_events: Vec<(GPv2SettlementEvents, Log)>,
) -> Result<Vec<(EventIndex, Event)>> {
    contract_events
        .into_iter()
        .filter_map(|(event, log)| {
            let log = ValidatedLog::try_from(log).ok()?;
            match event {
                GPv2SettlementEvents::Trade(event) => Some(convert_trade(&event, log)),
                GPv2SettlementEvents::Settlement(event) => {
                    Some(Ok(convert_settlement(&event, log)))
                }
                GPv2SettlementEvents::OrderInvalidated(event) => {
                    Some(convert_invalidation(&event, log))
                }
                GPv2SettlementEvents::PreSignature(event) => {
                    Some(convert_presignature(&event, log))
                }
                // TODO: handle new events
                GPv2SettlementEvents::Interaction(_) => None,
            }
        })
        .collect::<Result<Vec<_>>>()
}

struct ValidatedLog {
    block: i64,
    tx_hash: TransactionHash,
    log_index: i64,
}

impl TryFrom<Log> for ValidatedLog {
    type Error = anyhow::Error;

    fn try_from(log: Log) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            block: log
                .block_number
                .context("missing block_number")?
                .try_into()
                .context("could not convert block number to i64")?,
            tx_hash: log
                .transaction_hash
                .map(|hash| ByteArray(hash.0))
                .context("missing transaction_hash")?,
            log_index: log
                .log_index
                .context("missing log_index")?
                .try_into()
                .context("could not convert log index to i64")?,
        })
    }
}

impl From<ValidatedLog> for EventIndex {
    fn from(value: ValidatedLog) -> Self {
        Self {
            block_number: value.block,
            log_index: value.log_index,
        }
    }
}

pub async fn append_events(
    transaction: &mut PgTransaction<'_>,
    events: Vec<(GPv2SettlementEvents, Log)>,
) -> Result<()> {
    let _timer = super::Metrics::get()
        .database_queries
        .with_label_values(&["append_events"])
        .start_timer();

    let events = contract_to_db_events(events)?;
    database::events::append(transaction, &events)
        .await
        .context("append_events")?;
    Ok(())
}

pub async fn replace_events(
    transaction: &mut PgTransaction<'_>,
    events: Vec<(GPv2SettlementEvents, Log)>,
    from_block: u64,
) -> Result<()> {
    let _timer = super::Metrics::get()
        .database_queries
        .with_label_values(&["replace_events"])
        .start_timer();

    let events = contract_to_db_events(events)?;
    database::events::delete(transaction, from_block)
        .await
        .context("delete_events failed")?;
    database::events::append(transaction, events.as_slice())
        .await
        .context("insert_events failed")?;
    Ok(())
}

pub fn log_to_event_index(log: &Log) -> Option<EventIndex> {
    Some(EventIndex {
        block_number: log.block_number.and_then(|n| i64::try_from(n).ok())?,
        log_index: log.log_index.and_then(|n| i64::try_from(n).ok())?,
    })
}

pub fn bytes_to_order_uid(bytes: &[u8]) -> Result<OrderUid> {
    bytes
        .try_into()
        .context("order_uid has wrong number of bytes")
        .map(ByteArray)
}

fn convert_trade(trade: &GPv2Settlement::Trade, log: ValidatedLog) -> Result<(EventIndex, Event)> {
    let event = Trade {
        order_uid: bytes_to_order_uid(&trade.orderUid.0)?,
        sell_amount_including_fee: u256_to_big_decimal(&trade.sellAmount),
        buy_amount: u256_to_big_decimal(&trade.buyAmount),
        fee_amount: u256_to_big_decimal(&trade.feeAmount),
    };
    Ok((log.into(), Event::Trade(event)))
}

fn convert_settlement(
    settlement: &GPv2Settlement::Settlement,
    log: ValidatedLog,
) -> (EventIndex, Event) {
    let event = Settlement {
        solver: ByteArray(settlement.solver.into()),
        transaction_hash: log.tx_hash,
    };
    (log.into(), Event::Settlement(event))
}

fn convert_invalidation(
    invalidation: &GPv2Settlement::OrderInvalidated,
    log: ValidatedLog,
) -> Result<(EventIndex, Event)> {
    let event = Invalidation {
        order_uid: bytes_to_order_uid(invalidation.orderUid.as_ref())?,
    };
    Ok((log.into(), Event::Invalidation(event)))
}

fn convert_presignature(
    presignature: &GPv2Settlement::PreSignature,
    log: ValidatedLog,
) -> Result<(EventIndex, Event)> {
    let event = PreSignature {
        owner: ByteArray(presignature.owner.into()),
        order_uid: bytes_to_order_uid(presignature.orderUid.as_ref())?,
        signed: presignature.signed,
    };
    Ok((log.into(), Event::PreSignature(event)))
}
