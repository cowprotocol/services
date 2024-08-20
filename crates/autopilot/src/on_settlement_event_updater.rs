//! This module is responsible for updating the database, for each settlement
//! event that is emitted by the settlement contract.
//
// Each settlement transaction is expected to contain an auction id to uniquely
// identify the auction for which it was allowed to be brought onchain.
// This auction id is used to build the accociation between the settlement event
// and the auction in the database.
//
// Building of this asscociation happens in an always running background task.
//
// Alternatively we could change the event insertion code to do this but I (vk)
// would like to keep that code as fast as possible to not slow down event
// insertion which also needs to deal with reorgs. It is also nicer from a code
// organization standpoint.

// Another responsibility of this module is to observe the settlement and save
// data of interest to the database. This data includes surplus, taken fees, gas
// used etc.

use {
    crate::{domain::settlement, infra},
    anyhow::{anyhow, Context, Result},
    database::PgTransaction,
    std::sync::Arc,
    tokio::sync::Notify,
};

pub struct OnSettlementEventUpdater {
    inner: Arc<Inner>,
}

struct Inner {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    notify: Notify,
}

impl OnSettlementEventUpdater {
    /// Creates a new OnSettlementEventUpdater and asynchronously schedules the
    /// first update run.
    pub fn new(eth: infra::Ethereum, persistence: infra::Persistence) -> Self {
        let inner = Arc::new(Inner {
            eth,
            persistence,
            notify: Notify::new(),
        });
        let inner_clone = inner.clone();
        tokio::spawn(async move { Inner::listen_for_updates(inner_clone).await });
        Self { inner }
    }

    /// Deletes settlement_observations and order executions for the given range
    pub async fn delete_observations(
        transaction: &mut PgTransaction<'_>,
        from_block: u64,
    ) -> Result<()> {
        database::settlements::delete(transaction, from_block)
            .await
            .context("delete_settlement_observations")?;

        Ok(())
    }

    /// Schedules an update loop on a background thread
    pub fn schedule_update(&self) {
        self.inner.notify.notify_one();
    }
}

impl Inner {
    async fn listen_for_updates(self: Arc<Inner>) -> ! {
        loop {
            match self.update().await {
                Ok(true) => {
                    tracing::debug!("on settlement event updater ran and processed event");
                    // There might be more pending updates, continue immediately.
                    continue;
                }
                Ok(false) => {
                    tracing::debug!("on settlement event updater ran without update");
                }
                Err(err) => {
                    tracing::error!(?err, "on settlement event update task failed");
                }
            }
            self.notify.notified().await;
        }
    }

    /// Update database for settlement events that have not been processed yet.
    ///
    /// Returns whether an update was performed.
    async fn update(&self) -> Result<bool> {
        // Find a settlement event that has not been processed yet.
        let event = match self.persistence.get_settlement_without_auction().await? {
            Some(event) => event,
            None => return Ok(false),
        };

        tracing::debug!("updating settlement details for tx {:?}", event.transaction);

        // Reconstruct the settlement transaction based on the blockchain transaction
        // hash
        let transaction = match self.eth.transaction(event.transaction).await {
            Ok(transaction) => {
                let separator = self.eth.contracts().settlement_domain_separator();
                settlement::Transaction::new(&transaction, separator)
            }
            Err(err) => {
                tracing::warn!(hash = ?event.transaction, ?err, "no tx found");
                return Ok(false);
            }
        };

        // Build the <auction_id, settlement> association
        let (auction_id, settlement) = match transaction {
            Ok(transaction) => {
                let auction_id = transaction.auction_id;
                let settlement = match settlement::Settlement::new(transaction, &self.persistence)
                    .await
                {
                    Ok(settlement) => Some(settlement),
                    Err(err) if retryable(&err) => return Err(err.into()),
                    Err(err) => {
                        tracing::warn!(hash = ?event.transaction, ?auction_id, ?err, "invalid settlement");
                        None
                    }
                };
                (auction_id, settlement)
            }
            Err(err) => {
                tracing::warn!(hash = ?event.transaction, ?err, "invalid settlement transaction");
                // default values so we don't get stuck on invalid settlement transactions
                (0.into(), None)
            }
        };

        tracing::debug!(hash = ?event.transaction, ?auction_id, "saving settlement details for tx");

        if let Err(err) = self
            .persistence
            .save_settlement(event, auction_id, settlement.as_ref())
            .await
        {
            return Err(anyhow!(
                "save settlement: {:?}, {auction_id}, {err}",
                event.transaction
            ));
        }

        Ok(true)
    }
}

/// Whether OnSettlementEventUpdater loop should retry on the given error.
fn retryable(err: &settlement::Error) -> bool {
    match err {
        settlement::Error::Infra(_) => true,
        settlement::Error::InconsistentData(_) => false,
        settlement::Error::WrongEnvironment => false,
        settlement::Error::BuildingSolution(_) => false,
    }
}
