use {
    crate::{Amm, Metrics},
    anyhow::Context,
    contracts::{CowAmmLegacyHelper, cow_amm_legacy_helper::Event as CowAmmEvent},
    database::byte_array::ByteArray,
    ethcontract::{Address, errors::ExecutionError},
    ethrpc::block_stream::RangeInclusive,
    shared::event_handling::EventStoring,
    sqlx::PgPool,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::RwLock,
};

#[derive(Clone, Debug)]
pub(crate) struct Storage(Arc<Inner>);

impl Storage {
    pub(crate) fn new(deployment_block: u64, helper: CowAmmLegacyHelper, db: PgPool) -> Self {
        Self(Arc::new(Inner {
            cache: Default::default(),
            // make sure to start 1 block **before** the deployment to get all the events
            start_of_index: deployment_block - 1,
            helper,
            db,
        }))
    }

    pub(crate) async fn initialize_from_database(&self) -> anyhow::Result<()> {
        let mut ex = self.0.db.acquire().await?;
        let helper_address = ByteArray(self.0.helper.address().0);
        let db_amms = {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["cow_amm_fetch_by_helper"])
                .start_timer();

            database::cow_amms::fetch_by_helper_address(&mut ex, &helper_address).await?
        };

        if db_amms.is_empty() {
            return Ok(());
        }

        let mut processed_amms = Vec::new();
        for db_amm in db_amms {
            let amm_address = ethcontract::Address::from_slice(&db_amm.address.0);
            let amm = Amm::new(amm_address, &self.0.helper).await?;
            let block_number = u64::try_from(db_amm.block_number).context(format!(
                "db stored cow amm {:?} block number is not u64",
                db_amm.address
            ))?;
            processed_amms.push((block_number, Arc::new(amm)));
        }

        if !processed_amms.is_empty() {
            let count = processed_amms.len();
            let mut cache = self.0.cache.write().await;
            for (block_number, amm) in processed_amms {
                cache.entry(block_number).or_default().push(amm);
            }
            tracing::info!(count, ?helper_address, "initialized AMMs from database");
        }

        Ok(())
    }

    pub(crate) async fn cow_amms(&self) -> Vec<Arc<Amm>> {
        let lock = self.0.cache.read().await;
        lock.values()
            .flat_map(|amms| amms.iter().cloned())
            .collect()
    }

    pub(crate) async fn remove_amms(&self, amm_addresses: &[Address]) {
        let mut lock = self.0.cache.write().await;
        for (_, amms) in lock.iter_mut() {
            amms.retain(|amm| !amm_addresses.contains(amm.address()))
        }
    }
}

#[derive(Debug)]
struct Inner {
    /// Store indexed data associated to the indexed events type id.
    /// That type erasure allows us to index multiple concrete contracts
    /// in a single Registry to make for a nicer user facing API.
    cache: RwLock<HashMap<u64, Vec<Arc<Amm>>>>,
    /// The earliest block where indexing the contract makes sense.
    /// The contract did not emit any events before this block.
    start_of_index: u64,
    /// Helper contract to query required data from the cow amm.
    helper: CowAmmLegacyHelper,
    /// Database connection to persist CoW AMMs and the last indexed block.
    db: PgPool,
}

#[async_trait::async_trait]
impl EventStoring<ethcontract::Event<CowAmmEvent>> for Storage {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<CowAmmEvent>>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        {
            let mut ex = self.0.db.acquire().await?;
            let blocks = (*range.start()..=*range.end())
                .map(|block| i64::try_from(block).context("block number is not u64"))
                .collect::<anyhow::Result<Vec<_>>>()?;
            database::cow_amms::delete_by_blocks(&mut ex, &blocks).await?;
        }

        {
            let cache = &mut *self.0.cache.write().await;
            for key in *range.start()..=*range.end() {
                cache.remove(&key);
            }
        }

        self.append_events(events).await
    }

    /// Apply all the events to the given CoW AMM registry and update the
    /// internal registry
    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<CowAmmEvent>>,
    ) -> anyhow::Result<()> {
        let mut processed_events = Vec::with_capacity(events.len());
        for event in events {
            let Some(meta) = event.meta else {
                tracing::warn!(?event, "event does not contain required meta data");
                continue;
            };

            let CowAmmEvent::CowammpoolCreated(cow_amm) = event.data;
            let cow_amm = cow_amm.amm;
            match Amm::new(cow_amm, &self.0.helper).await {
                Ok(amm) => processed_events.push((meta.block_number, Arc::new(amm))),
                Err(err) if matches!(&err.inner, ExecutionError::Web3(_)) => {
                    // Abort completely to later try the entire block range again.
                    // That keeps the cache in a consistent state and avoids indexing
                    // the same event multiple times which would result in duplicate amms.
                    tracing::debug!(?cow_amm, ?err, "retryable error");
                    return Err(err.into());
                }
                Err(err) => {
                    tracing::info!(?cow_amm, ?err, "helper contract does not support amm");
                    continue;
                }
            };
        }

        if !processed_events.is_empty() {
            let db_amms = processed_events
                .iter()
                .filter_map(|(block_number, amm)| {
                    amm.as_ref()
                        .try_to_db_domain(*block_number, self.0.helper.address())
                        .map_err(|err| {
                            tracing::warn!(
                                ?err,
                                ?amm,
                                ?block_number,
                                helper = ?self.0.helper.address(),
                                "failed to convert amm to db domain"
                            );
                            err
                        })
                        .ok()
                })
                .collect::<Vec<database::cow_amms::CowAmm>>();
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["cow_amms_upsert_batched"])
                .start_timer();

            let mut ex = self.0.db.begin().await?;
            database::cow_amms::upsert_batched(&mut ex, &db_amms).await?;
        }

        // Update cache
        let cache = &mut *self.0.cache.write().await;
        for (block, amm) in processed_events {
            tracing::info!(cow_amm = ?amm.address(), "indexed new cow amm");
            cache.entry(block).or_default().push(amm);
        }

        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let mut ex = self.0.db.acquire().await?;
        database::last_indexed_blocks::fetch(&mut ex, &self.0.helper.address().to_string())
            .await?
            .map(|block| block.try_into().context("last block is not u64"))
            .unwrap_or(Ok(self.0.start_of_index))
    }

    async fn persist_last_indexed_block(&mut self, latest_block: u64) -> anyhow::Result<()> {
        let mut ex = self.0.db.acquire().await?;
        database::last_indexed_blocks::update(
            &mut ex,
            &self.0.helper.address().to_string(),
            i64::try_from(latest_block).context("last block is not u64")?,
        )
        .await?;
        Ok(())
    }
}
