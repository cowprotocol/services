use {
    crate::{Amm, Metrics},
    alloy::{primitives::Address, rpc::types::Log},
    anyhow::Context,
    contracts::cow_amm::{
        CowAmmLegacyHelper,
        CowAmmLegacyHelper::CowAmmLegacyHelper::CowAmmLegacyHelperEvents as CowAmmEvent,
    },
    database::byte_array::ByteArray,
    ethrpc::block_stream::RangeInclusive,
    shared::event_handling::EventStoring,
    sqlx::PgPool,
    std::{collections::HashMap, sync::Arc},
    tokio::sync::RwLock,
};

#[derive(Clone, Debug)]
pub(crate) struct Storage(Arc<Inner>);

impl Storage {
    pub(crate) async fn new(
        deployment_block: u64,
        helper: CowAmmLegacyHelper::Instance,
        factory_address: Address,
        db: PgPool,
    ) -> Self {
        let self_ = Self(Arc::new(Inner {
            cache: Default::default(),
            factory_address,
            // make sure to start 1 block **before** the deployment to get all the events
            start_of_index: deployment_block - 1,
            helper,
            db,
        }));

        if let Err(err) = self_.initialize_from_database().await {
            tracing::error!(
                ?err,
                ?factory_address,
                "failed to initialize AMM cache from database"
            );
        }

        self_
    }

    async fn initialize_from_database(&self) -> anyhow::Result<()> {
        let mut ex = self.0.db.acquire().await?;
        let factory_address = ByteArray(self.0.factory_address.0.0);
        let db_amms = {
            let _timer = Metrics::get()
                .database_queries
                .with_label_values(&["cow_amm_fetch_by_helper"])
                .start_timer();

            database::cow_amms::fetch_by_factory_address(&mut ex, &factory_address).await?
        };

        if db_amms.is_empty() {
            return Ok(());
        }

        let amm_process_tasks = db_amms.into_iter().map(|db_amm| async move {
            let amm_address = alloy::primitives::Address::from_slice(&db_amm.address.0);
            let amm = Amm::new(amm_address, &self.0.helper).await?;
            let block_number = u64::try_from(db_amm.block_number).context(format!(
                "db stored cow amm {:?} block number is not u64",
                db_amm.address
            ))?;

            Ok::<(u64, Arc<Amm>), anyhow::Error>((block_number, Arc::new(amm)))
        });
        let processed_amms = futures::future::try_join_all(amm_process_tasks).await?;

        let count = processed_amms.len();
        let db_amms = processed_amms
            .iter()
            .map(|(_, amm)| *amm.address())
            .collect::<Vec<_>>();
        let mut cache = self.0.cache.write().await;
        for (block_number, amm) in processed_amms {
            cache.entry(block_number).or_default().push(amm);
        }
        tracing::info!(
            count,
            ?factory_address,
            ?db_amms,
            "initialized AMMs from database"
        );

        Ok(())
    }

    pub(crate) async fn cow_amms(&self) -> Vec<Arc<Amm>> {
        let lock = self.0.cache.read().await;
        lock.values()
            .flat_map(|amms| amms.iter().cloned())
            .collect()
    }

    pub(crate) async fn remove_amms(&self, amm_addresses: &[alloy::primitives::Address]) {
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
    /// Address of the factory contract that deployed the AMMs.
    factory_address: Address,
    /// Helper contract to query required data from the cow amm.
    helper: CowAmmLegacyHelper::Instance,
    /// Database connection to persist CoW AMMs and the last indexed block.
    db: PgPool,
}

#[async_trait::async_trait]
impl EventStoring<(CowAmmEvent, Log)> for Storage {
    async fn replace_events(
        &mut self,
        events: Vec<(CowAmmEvent, Log)>,
        range: RangeInclusive<u64>,
    ) -> anyhow::Result<()> {
        {
            let mut ex = self.0.db.acquire().await?;
            let start_block = i64::try_from(*range.start()).context("start block is not i64")?;
            let end_block = i64::try_from(*range.end()).context("end block is not i64")?;
            let factory_address = ByteArray(self.0.factory_address.0.0);
            database::cow_amms::delete_by_block_range(
                &mut ex,
                &factory_address,
                start_block,
                end_block,
            )
            .await?;
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
    async fn append_events(&mut self, events: Vec<(CowAmmEvent, Log)>) -> anyhow::Result<()> {
        let mut processed_events = Vec::with_capacity(events.len());
        for (event, log) in events {
            let (Some(block_number), Some(tx_hash)) = (log.block_number, log.transaction_hash)
            else {
                tracing::warn!(?event, "event does not contain required meta data");
                continue;
            };

            let CowAmmEvent::COWAMMPoolCreated(cow_amm) = event;
            let cow_amm = cow_amm.amm;
            match Amm::new(cow_amm, &self.0.helper).await {
                Ok(amm) => processed_events.push((block_number, tx_hash, Arc::new(amm))),
                Err(err) if matches!(&err, alloy::contract::Error::TransportError(_)) => {
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
                .filter_map(|(block_number, tx_hash, amm)| {
                    amm.as_ref()
                        .try_to_db_type(*block_number, self.0.factory_address, *tx_hash)
                        .inspect_err(|err| {
                            tracing::warn!(
                                ?err,
                                ?amm,
                                ?block_number,
                                ?tx_hash,
                                helper = ?self.0.helper.address(),
                                "failed to convert amm to db domain"
                            );
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
            ex.commit().await?;
        }

        // Update cache
        let cache = &mut *self.0.cache.write().await;
        for (block, _tx_hash, amm) in processed_events {
            tracing::info!(cow_amm = ?amm.address(), "indexed new cow amm");
            cache.entry(block).or_default().push(amm);
        }

        Ok(())
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let mut ex = self.0.db.acquire().await?;
        database::last_indexed_blocks::fetch(&mut ex, &format!("{:#x}", self.0.factory_address))
            .await?
            .map(|block| block.try_into().context("last block is not u64"))
            .unwrap_or(Ok(self.0.start_of_index))
    }

    async fn persist_last_indexed_block(&mut self, latest_block: u64) -> anyhow::Result<()> {
        let mut ex = self.0.db.acquire().await?;
        database::last_indexed_blocks::update(
            &mut ex,
            &format!("{:#x}", self.0.factory_address),
            i64::try_from(latest_block).context("latest block is not u64")?,
        )
        .await?;
        Ok(())
    }
}
