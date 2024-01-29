use {
    crate::{
        boundary,
        domain::{self},
        infra::{
            self,
            persistence::{self, transaction::Transaction},
        },
    },
    anyhow::Context,
    ethrpc::current_block::RangeInclusive,
};

pub mod contracts;

pub struct Events {
    persistence: infra::Persistence,
}

/// Error type for the events module.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("persistence error: {0:?}")]
    Persistence(#[from] persistence::Error),
    #[error("unexpected event data: {0:?}")]
    Encoding(anyhow::Error),
}

impl Events {
    pub fn new(persistence: infra::Persistence) -> Self {
        Self { persistence }
    }

    pub async fn latest_block(&self) -> Result<u64, Error> {
        self.persistence
            .latest_settlement_event_block()
            .await
            .map_err(Error::Persistence)
    }

    pub async fn append(
        &self,
        events: Vec<boundary::Event<boundary::events::GPv2Contract>>,
    ) -> Result<(), Error> {
        let mut tx = self.persistence.begin().await?;
        self.insert(&mut tx, events).await?;
        tx.commit().await.map_err(Error::Persistence)
    }

    pub async fn replace(
        &self,
        events: Vec<boundary::Event<boundary::events::GPv2Contract>>,
        range: RangeInclusive<u64>,
    ) -> Result<(), Error> {
        let mut tx = self.persistence.begin().await?;
        self.delete(&mut tx, range).await?;
        self.insert(&mut tx, events).await?;
        tx.commit().await.map_err(Error::Persistence)
    }

    async fn delete(&self, tx: &mut Transaction, range: RangeInclusive<u64>) -> Result<(), Error> {
        self.persistence
            .delete_settlement_events(tx, *range.start())
            .await
            .map_err(Error::Persistence)?;
        self.persistence
            .delete_trade_events(tx, *range.start())
            .await
            .map_err(Error::Persistence)?;
        self.persistence
            .delete_cancellation_events(tx, *range.start())
            .await
            .map_err(Error::Persistence)?;
        self.persistence
            .delete_presignature_events(tx, *range.start())
            .await
            .map_err(Error::Persistence)
    }

    async fn insert(
        &self,
        tx: &mut Transaction,
        events: Vec<boundary::Event<boundary::events::GPv2Contract>>,
    ) -> Result<(), Error> {
        let mut settlements = Vec::new();
        let mut trades = Vec::new();
        let mut cancellations = Vec::new();
        let mut presignatures = Vec::new();

        for event in events {
            let metadata = match event.meta {
                Some(meta) => meta,
                None => {
                    tracing::warn!(?event, "GPv2Contract event without meta data");
                    continue;
                }
            };
            match event.data {
                boundary::events::GPv2Contract::Settlement(event) => {
                    settlements.push(contracts::settlement::Settlement {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        tx_hash: metadata.transaction_hash,
                        solver: event.solver,
                    });
                }
                boundary::events::GPv2Contract::Trade(event) => {
                    trades.push(contracts::settlement::Trade {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        uid: uid(&event.order_uid.0)?,
                        sell_amount_including_fee: event.sell_amount,
                        buy_amount: event.buy_amount,
                        fee_amount: event.fee_amount,
                    });
                }
                boundary::events::GPv2Contract::OrderInvalidated(event) => {
                    cancellations.push(contracts::settlement::Cancellation {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        uid: uid(&event.order_uid.0)?,
                    });
                }
                boundary::events::GPv2Contract::PreSignature(event) => {
                    presignatures.push(contracts::settlement::PreSignature {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        uid: uid(&event.order_uid.0)?,
                        owner: event.owner,
                        signed: event.signed,
                    });
                }
                boundary::events::GPv2Contract::Interaction(_) => {}
            }
        }

        self.persistence
            .store_settlement_events(tx, settlements)
            .await
            .map_err(Error::Persistence)?;
        self.persistence
            .store_trade_events(tx, trades)
            .await
            .map_err(Error::Persistence)?;
        self.persistence
            .store_cancellation_events(tx, cancellations)
            .await
            .map_err(Error::Persistence)?;
        self.persistence
            .store_presignature_events(tx, presignatures)
            .await
            .map_err(Error::Persistence)
    }
}

fn uid(bytes: &[u8]) -> Result<domain::OrderUid, Error> {
    Ok(domain::OrderUid(
        bytes
            .try_into()
            .context("order_uid has wrong number of bytes")
            .map_err(Error::Encoding)?,
    ))
}
