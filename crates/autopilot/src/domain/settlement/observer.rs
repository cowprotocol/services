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
        loop {
            match self.single_update().await {
                Ok(IndexSuccess::IndexedSettlement) => {
                    tracing::debug!("on settlement event updater ran and processed event");
                    // There might be more pending updates, continue immediately.
                    continue;
                }
                Ok(IndexSuccess::NothingToDo) => {
                    tracing::debug!("on settlement event updater ran without update");
                    break;
                }
                Ok(IndexSuccess::SkippedInvalidTransaction) => {
                    tracing::warn!("stored default values for unindexable transaction");
                    break;
                }
                Err(err) => {
                    tracing::debug!(?err, "encountered retryable error");
                    continue;
                }
            }
        }
    }

    /// Update database for settlement events that have not been processed yet.
    ///
    /// Returns whether an update was performed.
    async fn single_update(&self) -> Result<IndexSuccess> {
        // Find a settlement event that has not been processed yet.
        let Some(event) = self
            .persistence
            .get_settlement_without_auction()
            .await
            .context("failed to fetch unprocessed tx from DB")?
        else {
            return Ok(IndexSuccess::NothingToDo);
        };

        tracing::debug!(tx = ?event.transaction, "found unprocessed settlement");

        let settlement_data = self
            .fetch_auction_data_for_transaction(event.transaction)
            .await?;
        self.persistence
            .save_settlement(event, settlement_data.as_ref())
            .await
            .context("failed to update settlement")?;

        match settlement_data {
            None => Ok(IndexSuccess::SkippedInvalidTransaction),
            Some(_) => Ok(IndexSuccess::IndexedSettlement),
        }
    }

    /// Inspects the calldata of the transaction, decodes the arguments, and
    /// finds off-chain data associated with it based on the attached auction_id
    /// bytes.
    async fn fetch_auction_data_for_transaction(
        &self,
        tx: eth::TxId,
    ) -> Result<Option<Settlement>> {
        let transaction = match self.eth.transaction(tx).await {
            Ok(transaction) => {
                let separator = self.eth.contracts().settlement_domain_separator();
                let settlement_contract = self.eth.contracts().settlement().address().into();
                settlement::Transaction::try_new(
                    &transaction,
                    separator,
                    settlement_contract,
                    self.eth.contracts().authenticator(),
                )
                .await
            }
            Err(err) => {
                return Err(anyhow!(format!(
                    "node could not find the transaction - tx: {tx:?}, err: {err:?}",
                )));
            }
        };

        match transaction {
            Ok(transaction) => {
                let auction_id = transaction.auction_id;
                match settlement::Settlement::new(transaction, &self.persistence, self.eth.chain())
                    .await
                {
                    Ok(settlement) => Ok(Some(settlement)),
                    Err(err) if retryable(&err) => Err(err.into()),
                    Err(err) => {
                        tracing::warn!(?tx, ?auction_id, ?err, "invalid settlement");
                        Ok(None)
                    }
                }
            }
            Err(err) => {
                match err {
                    settlement::transaction::Error::MissingCalldata => {
                        tracing::error!(?tx, ?err, "invalid settlement transaction");
                        Ok(None)
                    }
                    settlement::transaction::Error::MissingAuctionId
                    | settlement::transaction::Error::Decoding(_)
                    | settlement::transaction::Error::SignatureRecover(_)
                    | settlement::transaction::Error::OrderUidRecover(_)
                    | settlement::transaction::Error::MissingSolver => {
                        tracing::warn!(?tx, ?err, "invalid settlement transaction");
                        Ok(None)
                    }
                    settlement::transaction::Error::Authentication(_) => {
                        // This has to be a temporary error because the settlement contract
                        // guarantees that SOME allow listed contract executed the transaction.
                        Err(anyhow!(format!(
                            "could not determing solver address - err: {err:?}"
                        )))
                    }
                }
            }
        }
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
