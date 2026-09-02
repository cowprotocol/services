//! Coordinates a `/settle` call against a driver: enforces the block-based
//! submission deadline, races the driver response against the on-chain
//! observation of the resulting transaction, and journals both edges of the
//! call to the settlements-execution table.
//!
//! Extracted so the regular auction loop and the fast-path handler can share
//! the exact same submission machinery.

use {
    crate::{
        domain::settlement::{ExecutionEnded, ExecutionStarted},
        infra::{self, solvers::dto::settle},
        maintenance::{MaintenanceSync, SyncTarget},
    },
    eth_domain_types::{self as eth, TxId},
    futures::FutureExt,
    std::time::Duration,
    tracing::instrument,
};

pub struct Config {
    /// How long we wait for the driver to signal the settlement completed
    /// before considering the call timed out.
    pub max_settlement_transaction_wait: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum SettleError {
    #[error(transparent)]
    Other(anyhow::Error),
    #[error("settlement transaction await reached deadline")]
    Timeout,
}

pub struct SettleCallCoordinator {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    maintenance: MaintenanceSync,
    config: Config,
}

impl SettleCallCoordinator {
    pub fn new(
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        maintenance: MaintenanceSync,
        config: Config,
    ) -> Self {
        Self {
            eth,
            persistence,
            maintenance,
            config,
        }
    }

    /// Sends a `/settle` request to `driver` and returns the tx id of the
    /// resulting settlement once it is mined (or an error if the submission
    /// deadline was crossed first).
    pub async fn settle(
        &self,
        driver: &infra::Driver,
        solver: eth::Address,
        solution_uid: usize,
        request: settle::Request,
    ) -> Result<TxId, SettleError> {
        let auction_id = request.auction_id;
        let deadline = request.submission_deadline_latest_block;

        let settle = async move {
            let current_block = self.eth.current_block().borrow().number;
            anyhow::ensure!(
                current_block < request.submission_deadline_latest_block,
                "submission deadline was missed"
            );

            self.store_execution_started(
                request.auction_id,
                solver,
                solution_uid,
                current_block,
                request.submission_deadline_latest_block,
            );
            driver
                .settle(&request, self.config.max_settlement_transaction_wait)
                .await
        }
        .boxed();

        let wait_for_settlement_transaction = self
            .wait_for_settlement_transaction(auction_id, solver, deadline, solution_uid)
            .boxed();

        // Wait for either the settlement transaction to be mined or the driver
        // returned a result.
        let result = match futures::future::select(wait_for_settlement_transaction, settle).await {
            futures::future::Either::Left((res, _)) => res,
            futures::future::Either::Right((driver_result, wait_for_settlement_transaction)) => {
                match driver_result {
                    Ok(_) => wait_for_settlement_transaction.await,
                    Err(err) => Err(SettleError::Other(err)),
                }
            }
        };

        self.store_execution_ended(solver, auction_id, solution_uid, &result);

        result
    }

    /// Stores settlement execution started event in the DB in a background
    /// task to not block the caller.
    fn store_execution_started(
        &self,
        auction_id: i64,
        solver: eth::Address,
        solution_uid: usize,
        start_block: u64,
        deadline_block: u64,
    ) {
        let persistence = self.persistence.clone();
        tokio::spawn(async move {
            let execution_started = ExecutionStarted {
                auction_id,
                solver,
                solution_uid,
                start_timestamp: chrono::Utc::now(),
                start_block,
                deadline_block,
            };

            if let Err(err) = persistence
                .store_settlement_execution_started(execution_started)
                .await
            {
                tracing::error!(?err, "failed to store settlement execution event");
            }
        });
    }

    /// Stores settlement execution ended event in the DB in a background task
    /// to not block the caller.
    fn store_execution_ended(
        &self,
        solver: eth::Address,
        auction_id: i64,
        solution_uid: usize,
        result: &Result<TxId, SettleError>,
    ) {
        let end_timestamp = chrono::Utc::now();
        let current_block = self.eth.current_block().borrow().number;
        let persistence = self.persistence.clone();
        let outcome = match result {
            Ok(_) => "success".to_string(),
            Err(SettleError::Timeout) => "timeout".to_string(),
            Err(SettleError::Other(err)) => format!("driver failed: {err}"),
        };

        tokio::spawn(async move {
            let execution_ended = ExecutionEnded {
                auction_id,
                solver,
                solution_uid,
                end_timestamp,
                end_block: current_block,
                outcome,
            };
            if let Err(err) = persistence
                .store_settlement_execution_ended(execution_ended)
                .await
            {
                tracing::error!(?err, "failed to update settlement execution event");
            }
        });
    }

    /// Tries to find a `settle` contract call originating from `solver` for
    /// this auction on chain. Returns `Timeout` once the submission deadline
    /// block passes without a match.
    #[instrument(skip_all)]
    async fn wait_for_settlement_transaction(
        &self,
        auction_id: i64,
        solver: eth::Address,
        submission_deadline_latest_block: u64,
        solution_uid: usize,
    ) -> Result<eth::TxId, SettleError> {
        let current = self.eth.current_block().borrow().number;
        tracing::debug!(%current, deadline=%submission_deadline_latest_block, %auction_id, "waiting for tag");
        loop {
            let block = ethrpc::block_stream::next_block(self.eth.current_block()).await;
            // Run maintenance to ensure the system processed the last available
            // block so it's possible to find the tx in the DB in
            // the next line.
            self.maintenance
                .wait_until_block_processed(SyncTarget::FullyProcessed(block.number))
                .await;

            match self
                .persistence
                .find_settlement_transaction(auction_id, solver, solution_uid)
                .await
            {
                Ok(Some(transaction)) => return Ok(transaction),
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        ?auction_id,
                        ?solver,
                        "failed to find settlement transaction"
                    );
                }
            }
            if block.number >= submission_deadline_latest_block {
                break;
            }
        }
        Err(SettleError::Timeout)
    }
}
