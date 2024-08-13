//! This module is responsible for updating the database, for each settlement
//! event that is emitted by the settlement contract.
//
// When we put settlement transactions on chain there is no reliable way to
// know the transaction hash because we can create multiple transactions with
// different gas prices. What we do know is the account and nonce that the
// transaction will have which is enough to uniquely identify it.
//
// We build an association between account-nonce and tx hash by backfilling
// settlement events with the account and nonce of their tx hash. This happens
// in an always running background task.
//
// Alternatively we could change the event insertion code to do this but I (vk)
// would like to keep that code as fast as possible to not slow down event
// insertion which also needs to deal with reorgs. It is also nicer from a code
// organization standpoint.

// 2. Inserting settlement observations
//
// see database/sql/V048__create_settlement_rewards.sql
//
// Surplus and fees calculation is based on:
// a) the mined transaction call data
// b) the auction external prices fetched from orderbook
// c) the orders fetched from orderbook
// After a transaction is mined we calculate the surplus and fees for each
// transaction and insert them into the database (settlement_observations
// table).

use {
    crate::{
        database::Postgres,
        domain::{eth, settlement},
        infra,
    },
    anyhow::{anyhow, Context, Result},
    database::PgTransaction,
    primitive_types::H256,
    std::sync::Arc,
    tokio::sync::Notify,
};

pub struct OnSettlementEventUpdater {
    inner: Arc<Inner>,
}

struct Inner {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    db: Postgres,
    notify: Notify,
}

impl OnSettlementEventUpdater {
    /// Creates a new OnSettlementEventUpdater and asynchronously schedules the
    /// first update run.
    pub fn new(eth: infra::Ethereum, db: Postgres, persistence: infra::Persistence) -> Self {
        let inner = Arc::new(Inner {
            eth,
            persistence,
            db,
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
        let mut ex = self
            .db
            .pool
            .begin()
            .await
            .context("acquire DB connection")?;
        let (hash, event) = match database::settlements::get_settlement_without_auction(&mut ex)
            .await
            .context("get_settlement_without_auction")?
        {
            Some(event) => (
                eth::TxId(H256(event.tx_hash.0)),
                eth::Event {
                    block: (event.block_number as u64).into(),
                    log_index: event.log_index as u64,
                },
            ),
            None => return Ok(false),
        };

        tracing::debug!("updating settlement details for tx {hash:?}");

        let transaction = match self.eth.transaction(hash).await {
            Ok(transaction) => {
                let separator = self.eth.contracts().settlement_domain_separator();
                settlement::Transaction::new(&transaction, separator)
            }
            Err(err) => {
                tracing::warn!(?hash, ?err, "no tx found");
                return Ok(false);
            }
        };

        let (auction_id, settlement) = match transaction {
            Ok(tx) => {
                let auction_id = tx.auction_id;
                let settlement = match settlement::Settlement::new(tx, &self.persistence).await {
                    Ok(settlement) => Some(settlement),
                    Err(err) if retryable(&err) => return Err(err.into()),
                    Err(err) => {
                        tracing::warn!(?hash, ?auction_id, ?err, "invalid settlement");
                        None
                    }
                };
                (auction_id, settlement)
            }
            Err(err) => {
                tracing::warn!(?hash, ?err, "invalid settlement transaction");
                // default values so we don't get stuck on invalid settlement transactions
                (0.into(), None)
            }
        };

        tracing::debug!(?hash, ?auction_id, "updating settlement details for tx");

        if let Err(err) = self
            .persistence
            .save_settlement(event, auction_id, settlement.as_ref())
            .await
        {
            return Err(anyhow!("save settlement: {hash:?}, {auction_id}, {err}"));
        }

        Ok(true)
    }
}

fn retryable(err: &settlement::Error) -> bool {
    match err {
        settlement::Error::Infra(_) => true,
        settlement::Error::InconsistentData(_) => false,
        settlement::Error::WrongEnvironment => false,
        settlement::Error::BuildingSolution(_) => false,
        settlement::Error::BuildingScore(_) => false,
        settlement::Error::SolverMismatch {
            expected: _,
            got: _,
        } => false,
    }
}
