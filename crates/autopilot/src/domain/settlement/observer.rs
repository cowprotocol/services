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
    ethrpc::alloy::conversions::IntoLegacy,
    futures::StreamExt,
    std::time::Duration,
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

    /// Post processes all outstanding settlements. This involves decoding the
    /// settlement details from the transaction and associating it with a
    /// solution proposed by a solver for the auction specified at the end of
    /// the transaction call data. If no solution can be found a dummy mapping
    /// gets saved to mark the settlement as processed. This can happen when a
    /// solver submits a solution despite not winning or if the settlement
    /// belongs to an auction that was arbitrated in another environment (i.e.
    /// prod vs. staging).
    pub async fn post_process_outstanding_settlement_transactions(&self) {
        let settlements = Self::retry_with_sleep(|| async move {
            self.persistence
                .get_settlements_without_auction()
                .await
                .inspect_err(|err| {
                    tracing::warn!(?err, "failed to fetch unprocessed settlements from DB")
                })
        })
        .await
        .unwrap_or_default();

        if settlements.is_empty() {
            tracing::debug!("no unprocessed settlements found");
            return;
        }

        // On mainnet it's common to have multiple settlements in the
        // same block. So even if we process every block immediately,
        // we should still post-process multiple settlements concurrently.
        const MAX_CONCURRENCY: usize = 10;
        futures::stream::iter(settlements)
            .for_each_concurrent(MAX_CONCURRENCY, |settlement| async move {
                tracing::debug!(tx = ?settlement.transaction, "start post processing of settlement");
                match Self::retry_with_sleep(|| self.post_process_settlement(settlement)).await {
                    Some(_) =>  tracing::debug!(tx = ?settlement.transaction, "successfully post-processed settlement"),
                    None => tracing::warn!(tx = ?settlement.transaction, "gave up on post-processing settlement"),
                }
            })
            .await;
    }

    async fn post_process_settlement(&self, settlement: eth::SettlementEvent) -> Result<()> {
        let settlement_data = self
            .fetch_auction_data_for_transaction(settlement.transaction)
            .await?;
        self.persistence
            .save_settlement(settlement, settlement_data.as_ref())
            .await
            .context("failed to update settlement")
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
                let settlement_contract = self
                    .eth
                    .contracts()
                    .settlement()
                    .address()
                    .into_legacy()
                    .into();
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
                    Err(settlement::Error::Infra(err)) => {
                        // bubble up retryable error
                        Err(err)
                    }
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

    async fn retry_with_sleep<F, OK, ERR>(future: impl Fn() -> F) -> Option<OK>
    where
        F: Future<Output = Result<OK, ERR>>,
        ERR: std::fmt::Debug,
    {
        const MAX_RETRIES: usize = 5;

        let mut tries = 0;
        while tries < MAX_RETRIES {
            match future().await {
                Ok(res) => return Some(res),
                Err(err) => {
                    tracing::warn!(try = tries, ?err, "failed to execute future");
                    tries += 1;
                    // wait a little to give temporary errors a chance to resolve themselves
                    const TEMP_ERROR_BACK_OFF: Duration = Duration::from_millis(100);
                    tokio::time::sleep(TEMP_ERROR_BACK_OFF).await;
                }
            }
        }
        None
    }
}
