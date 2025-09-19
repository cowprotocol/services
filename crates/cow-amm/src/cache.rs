use {
    crate::Amm,
    anyhow::Context,
    contracts::{CowAmmLegacyHelper, cow_amm_legacy_helper::Event as CowAmmEvent},
    ethcontract::{Address, errors::ExecutionError},
    ethrpc::block_stream::RangeInclusive,
    shared::event_handling::EventStoring,
    sqlx::PgPool,
    std::{collections::BTreeMap, sync::Arc},
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
        let helper_address = database::byte_array::ByteArray(self.0.helper.address().0);
        let db_amms = database::cow_amms::fetch_by_helper(&mut ex, &helper_address).await?;

        if db_amms.is_empty() {
            return Ok(());
        }

        let mut processed_amms = Vec::new();
        for db_amm in db_amms {
            let amm_address = ethcontract::Address::from_slice(&db_amm.address.0);
            match Amm::new(amm_address, &self.0.helper).await {
                Ok(amm) => {
                    processed_amms.push(Arc::new(amm));
                }
                Err(err) => {
                    tracing::warn!(?amm_address, ?err, "failed to initialize AMM from database");
                }
            }
        }

        // @todo: refine it
        if !processed_amms.is_empty() {
            let count = processed_amms.len();
            let mut cache = self.0.cache.write().await;
            cache.insert(self.0.start_of_index + 1, processed_amms);
            tracing::info!(count, "initialized AMMs from database");
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
    cache: RwLock<BTreeMap<u64, Vec<Arc<Amm>>>>,
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
        // Collect AMM addresses to delete from database
        let amm_addresses_to_delete = {
            let cache = &*self.0.cache.read().await;
            let mut addresses = Vec::new();
            for key in *range.start()..=*range.end() {
                if let Some(amms) = cache.get(&key) {
                    for amm in amms {
                        addresses.push(database::byte_array::ByteArray(amm.address().0));
                    }
                }
            }
            addresses
        };

        // Delete AMMs from database
        if !amm_addresses_to_delete.is_empty() {
            let mut ex = self.0.db.acquire().await?;
            database::cow_amms::delete_by_addresses(&mut ex, &amm_addresses_to_delete).await?;
        }

        // Remove the Cow AMM events from cache in the given range
        {
            let cache = &mut *self.0.cache.write().await;
            for key in *range.start()..=*range.end() {
                cache.remove(&key);
            }
        }

        // Apply all the new events
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

        // Persist AMMs to database
        if !processed_events.is_empty() {
            let db_amms = processed_events
                .iter()
                .map(|(_, amm)| amm.as_ref().into())
                .collect::<Vec<database::cow_amms::CowAmm>>();
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
        let cache = self.0.cache.read().await;

        match cache.last_key_value() {
            Some((block, _amms)) => Ok(*block),
            None => {
                let mut ex = self.0.db.acquire().await?;
                database::last_indexed_blocks::fetch(&mut ex, &self.0.helper.address().to_string())
                    .await?
                    .map(|block| block.try_into().context("last block is not u64"))
                    .unwrap_or(Ok(self.0.start_of_index))
            }
        }
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
