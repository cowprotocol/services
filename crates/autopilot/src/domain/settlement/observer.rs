//! This module is responsible for updating the database, for each settlement
//! event that is emitted by the settlement contract.
//
// Each settlement transaction is expected to contain an auction id to uniquely
// identify the auction for which it was allowed to be brought onchain.
// This auction id is used to build the accociation between the settlement event
// and the auction in the database.
//
// Another responsibility of this module is to observe the settlement and save
// data of interest to the database. This data includes surplus, taken fees, gas
// used etc.

use {
    crate::{domain::settlement, infra},
    anyhow::{anyhow, Result},
};

#[derive(Clone)]
pub struct Observer {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
}

impl Observer {
    /// Creates a new Observer and asynchronously schedules the first update
    /// run.
    pub fn new(eth: infra::Ethereum, persistence: infra::Persistence) -> Self {
        Self { eth, persistence }
    }

    /// Fetches all the available missing data needed for bookkeeping.
    /// This needs to get called after indexing a new settlement event
    /// since this code needs that data to already be present in the DB.
    pub async fn update(&self) {
        loop {
            match self.single_update().await {
                Ok(true) => {
                    tracing::debug!("on settlement event updater ran and processed event");
                    // There might be more pending updates, continue immediately.
                    continue;
                }
                Ok(false) => {
                    tracing::debug!("on settlement event updater ran without update");
                    break;
                }
                Err(err) => {
                    tracing::error!(?err, "on settlement event update task failed");
                    break;
                }
            }
        }
    }

    /// Update database for settlement events that have not been processed yet.
    ///
    /// Returns whether an update was performed.
    async fn single_update(&self) -> Result<bool> {
        // Find a settlement event that has not been processed yet.
        let Some(event) = self.persistence.get_settlement_without_auction().await? else {
            return Ok(false);
        };

        tracing::debug!(tx = ?event.transaction, "updating settlement details");

        // Reconstruct the settlement transaction based on the transaction hash
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

/// Whether Observer loop should retry on the given error.
fn retryable(err: &settlement::Error) -> bool {
    match err {
        settlement::Error::Infra(_) => true,
        settlement::Error::InconsistentData(_) => false,
        settlement::Error::WrongEnvironment => false,
    }
}
