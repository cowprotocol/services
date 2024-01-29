use {
    crate::{boundary, database::Postgres, domain},
    anyhow::{anyhow, Context},
    chrono::Utc,
    database::{byte_array::ByteArray, events::insert_settlement},
    itertools::Itertools,
    number::conversions::u256_to_big_decimal,
    std::sync::Arc,
    tokio::time::Instant,
    tracing::Instrument,
};

pub mod cli;
pub mod dto;
pub mod transaction;

#[derive(Clone)]
pub struct Persistence {
    s3: Option<s3::Uploader>,
    postgres: Arc<Postgres>,
}

impl Persistence {
    pub async fn new(config: Option<s3::Config>, postgres: Arc<Postgres>) -> Self {
        Self {
            s3: match config {
                Some(config) => Some(s3::Uploader::new(config).await),
                None => None,
            },
            postgres,
        }
    }

    /// There is always only one `current` auction.
    ///
    /// This method replaces the current auction with the given one.
    ///
    /// If the given auction is successfully saved, it is also archived.
    pub async fn replace_current_auction(
        &self,
        auction: domain::Auction,
    ) -> Result<domain::AuctionId, Error> {
        let auction = dto::auction::from_domain(auction.clone());
        self.postgres
            .replace_current_auction(&auction)
            .await
            .map(|auction_id| {
                self.archive_auction(auction_id, auction);
                auction_id
            })
            .map_err(Error::DbError)
    }

    pub async fn solvable_orders(
        &self,
        min_valid_to: u32,
    ) -> Result<boundary::SolvableOrders, Error> {
        self.postgres
            .solvable_orders(min_valid_to)
            .await
            .map_err(Error::DbError)
    }

    /// Saves the given auction to storage for debugging purposes.
    ///
    /// There is no intention to retrieve this data programmatically.
    fn archive_auction(&self, id: domain::AuctionId, instance: dto::auction::Auction) {
        let Some(uploader) = self.s3.clone() else {
            return;
        };
        tokio::spawn(
            async move {
                match uploader.upload(id.to_string(), &instance).await {
                    Ok(key) => {
                        tracing::info!(?key, "uploaded auction to s3");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to upload auction to s3");
                    }
                }
            }
            .instrument(tracing::Span::current()),
        );
    }

    /// Saves the competition data to the DB
    pub async fn save_competition(&self, competition: &boundary::Competition) -> Result<(), Error> {
        self.postgres
            .save_competition(competition)
            .await
            .map_err(Error::DbError)
    }

    /// Inserts an order event for each order uid in the given set.
    /// Unique order uids are required to avoid inserting events with the same
    /// label within the same order_uid. If this function encounters an error it
    /// will only be printed. More elaborate error handling is not necessary
    /// because this is just debugging information.
    pub fn store_order_events(
        &self,
        order_uids: Vec<domain::OrderUid>,
        label: boundary::OrderEventLabel,
    ) {
        let db = self.postgres.clone();
        tokio::spawn(
            async move {
                let start = Instant::now();
                let events_count = order_uids.len();
                match boundary::store_order_events(&db, order_uids, label, Utc::now()).await {
                    Ok(_) => {
                        tracing::debug!(elapsed=?start.elapsed(), ?events_count, "stored order events");
                    }
                    Err(err) => {
                        tracing::warn!(?err, "failed to insert order events");
                    }
                }
            }
                .instrument(tracing::Span::current()),
        );
    }

    /// Saves the given fee policies to the DB as a single batch.
    pub async fn store_fee_policies(
        &self,
        auction_id: domain::AuctionId,
        fee_policies: Vec<(domain::OrderUid, Vec<domain::fee::Policy>)>,
    ) -> anyhow::Result<()> {
        let fee_policies = fee_policies
            .into_iter()
            .flat_map(|(order_uid, policies)| {
                policies
                    .into_iter()
                    .map(move |policy| dto::FeePolicy::from_domain(auction_id, order_uid, policy))
            })
            .collect_vec();

        let mut ex = self.postgres.pool.begin().await.context("begin")?;
        for chunk in fee_policies.chunks(self.postgres.config.insert_batch_size.get()) {
            crate::database::fee_policies::insert_batch(&mut ex, chunk.iter().cloned())
                .await
                .context("fee_policies::insert_batch")?;
        }

        ex.commit().await.context("commit")
    }

    /// Returns an atomic transaction object which can be used to guarantee
    /// multiple persistence operation happen consistently.
    pub async fn begin(&self) -> Result<transaction::Transaction, Error> {
        transaction::Transaction::begin(&self.postgres).await
    }

    /// Returns the latest block number for which settlement events have been
    /// saved.
    pub async fn latest_settlement_event_block(&self) -> Result<u64, Error> {
        let mut con = self
            .postgres
            .pool
            .acquire()
            .await
            .map_err(|e| Error::DbError(anyhow!("error acquiring connection {}", e)))?;
        let block_number = database::events::last_block(&mut con)
            .await
            .map_err(Error::Sql)?;
        block_number
            .try_into()
            .context("block number is negative")
            .map_err(Error::DataConsistency)
    }

    /// Saves the given settlement events to the DB.
    pub async fn store_settlement_events(
        &self,
        tx: &mut transaction::Transaction,
        events: Vec<domain::events::contracts::settlement::Settlement>,
    ) -> Result<(), Error> {
        for event in events {
            insert_settlement(
                &mut tx.inner,
                &database::events::EventIndex {
                    block_number: event.block_number as i64,
                    log_index: event.log_index as i64,
                },
                &database::events::Settlement {
                    solver: ByteArray(event.solver.0),
                    transaction_hash: ByteArray(event.tx_hash.0),
                },
            )
            .await
            .map_err(Error::Sql)?;
        }
        Ok(())
    }

    /// Saves the given pre-signature events to the DB.
    pub async fn store_presignature_events(
        &self,
        tx: &mut transaction::Transaction,
        events: Vec<domain::events::contracts::settlement::PreSignature>,
    ) -> Result<(), Error> {
        for event in events {
            database::events::insert_presignature(
                &mut tx.inner,
                &database::events::EventIndex {
                    block_number: event.block_number as i64,
                    log_index: event.log_index as i64,
                },
                &database::events::PreSignature {
                    owner: ByteArray(event.owner.0),
                    order_uid: ByteArray(event.uid.0),
                    signed: event.signed,
                },
            )
            .await
            .map_err(Error::Sql)?;
        }
        Ok(())
    }

    /// Saves the given trade events to the DB.
    pub async fn store_trade_events(
        &self,
        tx: &mut transaction::Transaction,
        events: Vec<domain::events::contracts::settlement::Trade>,
    ) -> Result<(), Error> {
        for event in events {
            database::events::insert_trade(
                &mut tx.inner,
                &database::events::EventIndex {
                    block_number: event.block_number as i64,
                    log_index: event.log_index as i64,
                },
                &database::events::Trade {
                    order_uid: ByteArray(event.uid.0),
                    sell_amount_including_fee: u256_to_big_decimal(
                        &event.sell_amount_including_fee,
                    ),
                    buy_amount: u256_to_big_decimal(&event.buy_amount),
                    fee_amount: u256_to_big_decimal(&event.fee_amount),
                },
            )
            .await
            .map_err(Error::Sql)?;
        }
        Ok(())
    }

    /// Saves the given on-chain cancellation events to the DB.
    pub async fn store_cancellation_events(
        &self,
        tx: &mut transaction::Transaction,
        events: Vec<domain::events::contracts::settlement::Cancellation>,
    ) -> Result<(), Error> {
        for event in events {
            database::events::insert_invalidation(
                &mut tx.inner,
                &database::events::EventIndex {
                    block_number: event.block_number as i64,
                    log_index: event.log_index as i64,
                },
                &database::events::Invalidation {
                    order_uid: ByteArray(event.uid.0),
                },
            )
            .await
            .map_err(Error::Sql)?;
        }
        Ok(())
    }

    /// Deletes all settlement events from the given block number onwards.
    pub async fn delete_settlement_events(
        &self,
        tx: &mut transaction::Transaction,
        from_block: u64,
    ) -> Result<(), Error> {
        database::events::delete_settlements(&mut tx.inner, from_block)
            .await
            .map_err(Error::Sql)
    }

    /// Deletes all pre-signature events from the given block number onwards.
    pub async fn delete_presignature_events(
        &self,
        tx: &mut transaction::Transaction,
        from_block: u64,
    ) -> Result<(), Error> {
        database::events::delete_presignatures(&mut tx.inner, from_block)
            .await
            .map_err(Error::Sql)
    }

    /// Deletes all trade events from the given block number onwards.
    pub async fn delete_trade_events(
        &self,
        tx: &mut transaction::Transaction,
        from_block: u64,
    ) -> Result<(), Error> {
        database::events::delete_trades(&mut tx.inner, from_block)
            .await
            .map_err(Error::Sql)
    }

    /// Deletes all cancellation events from the given block number onwards.
    pub async fn delete_cancellation_events(
        &self,
        tx: &mut transaction::Transaction,
        from_block: u64,
    ) -> Result<(), Error> {
        database::events::delete_invalidations(&mut tx.inner, from_block)
            .await
            .map_err(Error::Sql)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read or write data from database")]
    DbError(anyhow::Error),
    #[error("Error preparing SQL query")]
    Sql(sqlx::Error),
    #[error("Data consistency error")]
    DataConsistency(anyhow::Error),
}
