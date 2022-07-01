use super::Postgres;
use crate::conversions::*;
use anyhow::{anyhow, Context, Result};
use contracts::gpv2_settlement::{
    event_data::{
        OrderInvalidated as ContractInvalidation, PreSignature as ContractPreSignature,
        Settlement as ContractSettlement, Trade as ContractTrade,
    },
    Event as ContractEvent,
};
use ethcontract::{Event as EthContractEvent, EventMetadata, H160, H256, U256};
use futures::FutureExt;
use model::order::OrderUid;
use shared::event_handling::{EventIndex, EventStoring};
use sqlx::{Connection, Executor, Transaction};
use std::convert::TryInto;

#[derive(Debug)]
pub enum Event {
    Trade(Trade),
    Invalidation(Invalidation),
    Settlement(Settlement),
    PreSignature(PreSignature),
}

#[derive(Debug, Default)]
pub struct Trade {
    pub order_uid: OrderUid,
    pub sell_amount_including_fee: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
}

#[derive(Debug, Default)]
pub struct Invalidation {
    pub order_uid: OrderUid,
}

#[derive(Debug, Default)]
pub struct Settlement {
    pub solver: H160,
    pub transaction_hash: H256,
}

#[derive(Debug, Default)]
pub struct PreSignature {
    pub owner: H160,
    pub order_uid: OrderUid,
    pub signed: bool,
}

impl Postgres {
    // All insertions happen in one transaction.
    pub async fn append_events_(&self, events: Vec<(EventIndex, Event)>) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    append_events(transaction, events.as_slice())
                        .await
                        .context("append_events failed")
                }
                .boxed()
            })
            .await?;
        Ok(())
    }

    // The deletion and all insertions happen in one transaction.
    pub async fn replace_events_(
        &self,
        delete_from_block_number: u64,
        events: Vec<(EventIndex, Event)>,
    ) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    delete_events(transaction, delete_from_block_number)
                        .await
                        .context("delete_events failed")?;
                    append_events(transaction, events.as_slice())
                        .await
                        .context("insert_events failed")
                }
                .boxed()
            })
            .await?;
        Ok(())
    }
}

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

        const QUERY: &str = "\
            SELECT GREATEST( \
                (SELECT COALESCE(MAX(block_number), 0) FROM trades), \
                (SELECT COALESCE(MAX(block_number), 0) FROM settlements), \
                (SELECT COALESCE(MAX(block_number), 0) FROM invalidations), \
                (SELECT COALESCE(MAX(block_number), 0) FROM presignature_events));";
        let block_number: i64 = sqlx::query_scalar(QUERY)
            .fetch_one(&self.pool)
            .await
            .context("block_number_of_most_recent_event failed")?;
        block_number.try_into().context("block number is negative")
    }

    async fn append_events(&mut self, events: Vec<EthContractEvent<ContractEvent>>) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["append_events"])
            .start_timer();

        self.append_events_(contract_to_db_events(events)?).await
    }

    async fn replace_events(
        &mut self,
        events: Vec<EthContractEvent<ContractEvent>>,
        range: std::ops::RangeInclusive<shared::event_handling::BlockNumber>,
    ) -> Result<()> {
        let _timer = super::Metrics::get()
            .database_queries
            .with_label_values(&["replace_events"])
            .start_timer();

        self.replace_events_(range.start().to_u64(), contract_to_db_events(events)?)
            .await
    }
}

async fn delete_events(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    delete_from_block_number: u64,
) -> Result<(), sqlx::Error> {
    const QUERY_INVALIDATION: &str = "DELETE FROM invalidations WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY_INVALIDATION).bind(delete_from_block_number as i64))
        .await?;

    const QUERY_TRADE: &str = "DELETE FROM trades WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY_TRADE).bind(delete_from_block_number as i64))
        .await?;

    const QUERY_SETTLEMENTS: &str = "DELETE FROM settlements WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY_SETTLEMENTS).bind(delete_from_block_number as i64))
        .await?;

    const QUERY_PRESIGNATURES: &str = "DELETE FROM presignature_events WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY_PRESIGNATURES).bind(delete_from_block_number as i64))
        .await?;

    Ok(())
}

async fn append_events(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    events: &[(EventIndex, Event)],
) -> Result<(), sqlx::Error> {
    // TODO: there might be a more efficient way to do this like execute_many or COPY but my
    // tests show that even if we sleep during the transaction it does not block other
    // connections from using the database, so it's not high priority.
    for (index, event) in events {
        match event {
            Event::Trade(event) => insert_trade(transaction, index, event).await?,
            Event::Invalidation(event) => insert_invalidation(transaction, index, event).await?,
            Event::Settlement(event) => insert_settlement(transaction, index, event).await?,
            Event::PreSignature(event) => insert_presignature(transaction, index, event).await?,
        };
    }
    Ok(())
}

async fn insert_invalidation(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    index: &EventIndex,
    event: &Invalidation,
) -> Result<(), sqlx::Error> {
    // We use ON CONFLICT so that multiple updates running at the same do not error because of
    // events already existing. This can happen when multiple orderbook apis run in HPA.
    // See #444 .
    const QUERY: &str =
        "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3) \
         ON CONFLICT DO NOTHING;";
    transaction
        .execute(
            sqlx::query(QUERY)
                .bind(index.block_number as i64)
                .bind(index.log_index as i64)
                .bind(event.order_uid.0.as_ref()),
        )
        .await?;
    Ok(())
}

async fn insert_trade(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    index: &EventIndex,
    event: &Trade,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO trades (block_number, log_index, order_uid, sell_amount, buy_amount, fee_amount) VALUES ($1, $2, $3, $4, $5, $6) \
        ON CONFLICT DO NOTHING;";
    transaction
        .execute(
            sqlx::query(QUERY)
                .bind(index.block_number as i64)
                .bind(index.log_index as i64)
                .bind(event.order_uid.0.as_ref())
                .bind(u256_to_big_decimal(&event.sell_amount_including_fee))
                .bind(u256_to_big_decimal(&event.buy_amount))
                .bind(u256_to_big_decimal(&event.fee_amount)),
        )
        .await?;
    Ok(())
}

async fn insert_settlement(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    index: &EventIndex,
    event: &Settlement,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO settlements (tx_hash, block_number, log_index, solver) VALUES ($1, $2, $3, $4) \
        ON CONFLICT DO NOTHING;";
    transaction
        .execute(
            sqlx::query(QUERY)
                .bind(event.transaction_hash.as_bytes())
                .bind(index.block_number as i64)
                .bind(index.log_index as i64)
                .bind(event.solver.as_bytes()),
        )
        .await?;
    Ok(())
}

async fn insert_presignature(
    transaction: &mut Transaction<'_, sqlx::Postgres>,
    index: &EventIndex,
    event: &PreSignature,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "\
        INSERT INTO presignature_events (block_number, log_index, owner, order_uid, signed) VALUES ($1, $2, $3, $4, $5) \
        ON CONFLICT DO NOTHING;";
    transaction
        .execute(
            sqlx::query(QUERY)
                .bind(index.block_number as i64)
                .bind(index.log_index as i64)
                .bind(event.owner.as_ref())
                .bind(event.order_uid.0.as_ref())
                .bind(event.signed),
        )
        .await?;
    Ok(())
}

fn convert_trade(trade: &ContractTrade, meta: &EventMetadata) -> Result<(EventIndex, Event)> {
    let order_uid = OrderUid(
        trade
            .order_uid
            .0
            .as_slice()
            .try_into()
            .context("trade event order_uid has wrong number of bytes")?,
    );
    let event = Trade {
        order_uid,
        sell_amount_including_fee: trade.sell_amount,
        buy_amount: trade.buy_amount,
        fee_amount: trade.fee_amount,
    };
    Ok((EventIndex::from(meta), Event::Trade(event)))
}

fn convert_settlement(
    settlement: &ContractSettlement,
    meta: &EventMetadata,
) -> (EventIndex, Event) {
    let event = Settlement {
        solver: settlement.solver,
        transaction_hash: meta.transaction_hash,
    };
    (EventIndex::from(meta), Event::Settlement(event))
}

fn convert_invalidation(
    invalidation: &ContractInvalidation,
    meta: &EventMetadata,
) -> Result<(EventIndex, Event)> {
    let order_uid = OrderUid(
        invalidation
            .order_uid
            .0
            .as_slice()
            .try_into()
            .context("invalidation event order_uid has wrong number of bytes")?,
    );
    let event = Invalidation { order_uid };
    Ok((EventIndex::from(meta), Event::Invalidation(event)))
}

fn convert_presignature(
    presignature: &ContractPreSignature,
    meta: &EventMetadata,
) -> Result<(EventIndex, Event)> {
    let order_uid = OrderUid(
        presignature
            .order_uid
            .0
            .as_slice()
            .try_into()
            .context("trade event order_uid has wrong number of bytes")?,
    );
    let event = PreSignature {
        owner: presignature.owner,
        order_uid,
        signed: presignature.signed,
    };
    Ok((EventIndex::from(meta), Event::PreSignature(event)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_events() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();

        assert_eq!(db.last_event_block().await.unwrap(), 0);

        db.append_events_(vec![(
            EventIndex {
                block_number: 1,
                log_index: 0,
            },
            Event::Invalidation(Invalidation::default()),
        )])
        .await
        .unwrap();
        assert_eq!(db.last_event_block().await.unwrap(), 1);

        db.append_events_(vec![(
            EventIndex {
                block_number: 2,
                log_index: 0,
            },
            Event::Trade(Trade::default()),
        )])
        .await
        .unwrap();
        assert_eq!(db.last_event_block().await.unwrap(), 2);

        db.append_events_(vec![(
            EventIndex {
                block_number: 3,
                log_index: 0,
            },
            Event::PreSignature(PreSignature::default()),
        )])
        .await
        .unwrap();
        assert_eq!(db.last_event_block().await.unwrap(), 3);

        db.replace_events_(
            0,
            vec![(
                EventIndex {
                    block_number: 3,
                    log_index: 0,
                },
                Event::Invalidation(Invalidation::default()),
            )],
        )
        .await
        .unwrap();
        assert_eq!(db.last_event_block().await.unwrap(), 3);

        db.replace_events_(2, vec![]).await.unwrap();
        assert_eq!(db.last_event_block().await.unwrap(), 0);

        db.append_events_(vec![(
            EventIndex {
                block_number: 1,
                log_index: 2,
            },
            Event::Settlement(Settlement {
                solver: H160::from_low_u64_be(3),
                transaction_hash: H256::from_low_u64_be(4),
            }),
        )])
        .await
        .unwrap();

        assert_eq!(db.last_event_block().await.unwrap(), 1);

        db.replace_events_(1, vec![]).await.unwrap();
        assert_eq!(db.last_event_block().await.unwrap(), 0);
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_repeated_event_insert_ignored() {
        let db = Postgres::new("postgresql://").unwrap();
        database::clear_DANGER(&db.pool).await.unwrap();
        for _ in 0..2 {
            db.append_events_(vec![(
                EventIndex {
                    block_number: 2,
                    log_index: 0,
                },
                Event::Trade(Default::default()),
            )])
            .await
            .unwrap();
            db.append_events_(vec![(
                EventIndex {
                    block_number: 2,
                    log_index: 1,
                },
                Event::Invalidation(Default::default()),
            )])
            .await
            .unwrap();
            db.append_events_(vec![(
                EventIndex {
                    block_number: 2,
                    log_index: 2,
                },
                Event::Settlement(Default::default()),
            )])
            .await
            .unwrap();
            db.append_events_(vec![(
                EventIndex {
                    block_number: 2,
                    log_index: 3,
                },
                Event::PreSignature(Default::default()),
            )])
            .await
            .unwrap();
        }
        assert_eq!(db.last_event_block().await.unwrap(), 2);
    }
}
