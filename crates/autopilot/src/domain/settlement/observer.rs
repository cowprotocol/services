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
    crate::{
        domain::{
            eth,
            settlement::{self, Settlement},
        },
        infra,
    },
    anyhow::{Context, Result, anyhow},
    std::time::Duration,
};

#[derive(Clone)]
pub struct Observer {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
}

enum IndexSuccess {
    NothingToDo,
    IndexedSettlement,
    SkippedInvalidTransaction,
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
        const MAX_RETRIES: usize = 5;
        let mut attempts = 0;
        while attempts < MAX_RETRIES {
            match self.single_update().await {
                Ok(IndexSuccess::IndexedSettlement) => {
                    tracing::debug!("on settlement event updater ran and processed event");
                }
                Ok(IndexSuccess::SkippedInvalidTransaction) => {
                    tracing::warn!("stored default values for unindexable transaction");
                }
                Ok(IndexSuccess::NothingToDo) => {
                    tracing::debug!("on settlement event updater ran without update");
                    return;
                }
                Err(err) => {
                    tracing::debug!(?err, "encountered retryable error");
                    // wait a little to give temporary errors a chance to resolve themselves
                    const TEMP_ERROR_BACK_OFF: Duration = Duration::from_millis(100);
                    tokio::time::sleep(TEMP_ERROR_BACK_OFF).await;
                    attempts += 1;
                    continue;
                }
            }

            // everything worked fine -> reset our attempts for the next settlement
            attempts = 0;
        }
    }

    /// Update database for settlement events that have not been processed yet.
    ///
    /// Returns whether an update was performed.
    async fn single_update(&self) -> Result<IndexSuccess> {
        let Some(event) = self
            .persistence
            .get_settlement_without_auction()
            .await
            .context("failed to fetch unprocessed tx from DB")?
        else {
            return Ok(IndexSuccess::NothingToDo);
        };

        tracing::debug!(tx = ?event.transaction, "found unprocessed settlement");

        let settlements = self
            .fetch_multi_settlement_data_for_transaction(event.transaction)
            .await?;
        
        if settlements.is_empty() {
            self.persistence
                .save_settlement(event, None)
                .await
                .context("failed to update settlement")?;
            return Ok(IndexSuccess::SkippedInvalidTransaction);
        }

        if settlements.len() > 1 {
            tracing::info!(
                tx = ?event.transaction, 
                count = settlements.len(),
                "processing multi-settlement transaction"
            );
        }
        
        let all_events = self
            .persistence
            .get_all_unprocessed_settlements_for_transaction(event.transaction)
            .await
            .context("failed to fetch all settlement events for transaction")?;
        
        if all_events.len() != settlements.len() {
            tracing::warn!(
                tx = ?event.transaction,
                event_count = all_events.len(),
                settlement_count = settlements.len(),
                "mismatch between settlement events and settlements - processing available settlements"
            );
        }
        
        for (i, settlement) in settlements.into_iter().enumerate() {
            if let Some(settlement_event) = all_events.get(i) {
                self.persistence
                    .save_settlement(*settlement_event, Some(&settlement))
                    .await
                    .context("failed to update settlement")?;
            }
        }
        
        Ok(IndexSuccess::IndexedSettlement)
    }

    async fn fetch_multi_settlement_data_for_transaction(
        &self,
        tx: eth::TxId,
    ) -> Result<Vec<Settlement>> {
        let transaction = self.eth.transaction(tx).await
            .with_context(|| format!("node could not find the transaction - tx: {tx:?}"))?;
        
        let separator = self.eth.contracts().settlement_domain_separator();
        let settlement_contract = self.eth.contracts().settlement().address().into();
        let transaction = settlement::transaction::TransactionSettlements::try_new(
            &transaction,
            separator,
            settlement_contract,
            self.eth.contracts().authenticator(),
        )
        .await;

        match transaction {
            Ok(multi_transaction) => {
                let mut settlements = Vec::new();
                
                for settlement_data in &multi_transaction.settlements {
                    let single_transaction = settlement::transaction::Transaction::from_multi_settlement(
                        &multi_transaction,
                        settlement_data,
                    );

                    match settlement::Settlement::new(single_transaction, &self.persistence, self.eth.chain())
                        .await
                    {
                        Ok(settlement) => {
                            settlements.push(settlement);
                        }
                        Err(settlement::Error::Infra(err)) => {
                            // bubble up retryable error
                            return Err(err);
                        }
                        Err(err) => {
                            tracing::warn!(?tx, ?settlement_data.auction_id, ?err, "invalid settlement in multi-settlement transaction");
                            // Continue processing other settlements even if one fails
                        }
                    }
                }

                Ok(settlements)
            }
            Err(err) => {
                match err {
                    settlement::transaction::Error::Authentication(_) => {
                        // This is a temporary error because the authenticator service might be down or network issues.
                        // It resolves itself when the service comes back online or network is fixed
                        // and we return an error so the transaction gets retried later instead of marking it as permanently failed.
                        tracing::warn!(?tx, ?err, "could not determine solver address");
                        Err(anyhow!(format!(
                            "could not determine solver address - err: {err:?}"
                        )))
                    }
                    _ => {
                        // All other errors are treated as invalid settlement transactions
                        tracing::warn!(?tx, ?err, "invalid settlement transaction");
                        Ok(vec![])
                    }
                }
            }
        }
    }
}
