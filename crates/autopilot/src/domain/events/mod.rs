use {
    crate::{
        boundary,
        domain::{self, eth},
        infra::{
            self,
            persistence::{self, transaction::Transaction},
        },
    },
    anyhow::Context,
    ethrpc::current_block::RangeInclusive,
};

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

pub mod settlement {
    use super::*;

    /// An order was fully or partially traded.
    pub struct Trade {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub uid: domain::OrderUid,
        pub sell_amount_including_fee: eth::U256,
        pub buy_amount: eth::U256,
        pub fee_amount: eth::U256,
    }

    /// An order was cancelled on-chain.
    pub struct Cancellation {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub uid: domain::OrderUid,
    }

    /// A settlement was executed on-chain.
    pub struct Settlement {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub tx_hash: eth::H256,
        pub solver: eth::Address,
    }

    /// An order was signed on-chain or signature for an on-chain signed order
    /// has been revoked.
    pub struct PreSignature {
        pub block_number: u64,
        /// The index of the event in the block.
        pub log_index: usize,
        pub uid: domain::OrderUid,
        pub owner: eth::Address,
        pub signed: bool,
    }
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
                    settlements.push(settlement::Settlement {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        tx_hash: metadata.transaction_hash,
                        solver: event.solver,
                    });
                }
                boundary::events::GPv2Contract::Trade(event) => {
                    trades.push(settlement::Trade {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        uid: uid(&event.order_uid.0)?,
                        sell_amount_including_fee: event.sell_amount,
                        buy_amount: event.buy_amount,
                        fee_amount: event.fee_amount,
                    });
                }
                boundary::events::GPv2Contract::OrderInvalidated(event) => {
                    cancellations.push(settlement::Cancellation {
                        block_number: metadata.block_number,
                        log_index: metadata.log_index,
                        uid: uid(&event.order_uid.0)?,
                    });
                }
                boundary::events::GPv2Contract::PreSignature(event) => {
                    presignatures.push(settlement::PreSignature {
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

pub fn uid(bytes: &[u8]) -> Result<domain::OrderUid, Error> {
    Ok(domain::OrderUid(
        bytes
            .try_into()
            .context("order_uid has wrong number of bytes")
            .map_err(Error::Encoding)?,
    ))
}
