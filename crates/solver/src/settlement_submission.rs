pub mod archer_api;
pub mod archer_settlement;
mod dry_run;
pub mod flashbots_api;
pub mod flashbots_settlement;
mod gas_price_stream;
pub mod retry;
pub mod rpc;

use self::{
    archer_settlement::ArcherSolutionSubmitter, flashbots_settlement::FlashbotsSolutionSubmitter,
};
use crate::{metrics::SettlementSubmissionOutcome, settlement::Settlement};
use anyhow::{anyhow, Result};
use archer_api::ArcherApi;
use contracts::GPv2Settlement;
use ethcontract::{
    errors::{ExecutionError, MethodError},
    Account,
};
use flashbots_api::FlashbotsApi;
use gas_estimation::GasPriceEstimating;
use primitive_types::U256;
use shared::Web3;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use web3::types::TransactionReceipt;

const ESTIMATE_GAS_LIMIT_FACTOR: f64 = 1.2;
const GAS_PRICE_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

pub struct SolutionSubmitter {
    pub web3: Web3,
    pub contract: GPv2Settlement,
    pub gas_price_estimator: Arc<dyn GasPriceEstimating>,
    // for gas price estimation
    pub target_confirm_time: Duration,
    pub gas_price_cap: f64,
    pub transaction_strategy: TransactionStrategy,
}

pub enum TransactionStrategy {
    ArcherNetwork {
        archer_api: ArcherApi,
        max_confirm_time: Duration,
    },
    Flashbots {
        flashbots_api: FlashbotsApi,
        max_confirm_time: Duration,
        flashbots_tip: f64,
    },
    CustomNodes(Vec<Web3>),
    DryRun,
}

impl SolutionSubmitter {
    /// Submits a settlement transaction to the blockchain, returning the hash
    /// of the successfully mined transaction.
    ///
    /// Errors if the transaction timed out, or an inner error was encountered
    /// during submission.
    pub async fn settle(
        &self,
        settlement: Settlement,
        gas_estimate: U256,
        account: Account,
    ) -> Result<TransactionReceipt, SubmissionError> {
        match &self.transaction_strategy {
            TransactionStrategy::CustomNodes(nodes) => {
                rpc::submit(
                    nodes,
                    account,
                    &self.contract,
                    self.gas_price_estimator.as_ref(),
                    self.target_confirm_time,
                    self.gas_price_cap,
                    settlement,
                    gas_estimate,
                )
                .await
            }
            TransactionStrategy::ArcherNetwork {
                archer_api,
                max_confirm_time,
            } => {
                let submitter = ArcherSolutionSubmitter::new(
                    &self.web3,
                    &self.contract,
                    &account,
                    archer_api,
                    self.gas_price_estimator.as_ref(),
                    self.gas_price_cap,
                )?;
                let result = submitter
                    .submit(
                        self.target_confirm_time,
                        SystemTime::now() + *max_confirm_time,
                        settlement,
                        gas_estimate,
                    )
                    .await;
                result?.ok_or(SubmissionError::Timeout)
            }
            TransactionStrategy::Flashbots {
                flashbots_api,
                max_confirm_time,
                flashbots_tip,
            } => {
                let submitter = FlashbotsSolutionSubmitter::new(
                    &self.web3,
                    &self.contract,
                    &account,
                    flashbots_api,
                    self.gas_price_estimator.as_ref(),
                    self.gas_price_cap,
                )?;
                submitter
                    .submit(
                        self.target_confirm_time,
                        SystemTime::now() + *max_confirm_time,
                        settlement,
                        gas_estimate,
                        *flashbots_tip,
                    )
                    .await
            }
            TransactionStrategy::DryRun => {
                Ok(dry_run::log_settlement(account, &self.contract, settlement).await?)
            }
        }
    }
}

/// An error during settlement submission.
#[derive(Debug)]
pub enum SubmissionError {
    /// The transaction reverted.
    Revert(Option<String>),
    /// The settlement submission timed out.
    Timeout,
    /// An error occured.
    Other(anyhow::Error),
}

impl SubmissionError {
    /// Returns the outcome for use with metrics.
    pub fn as_outcome(&self) -> SettlementSubmissionOutcome {
        match self {
            Self::Timeout => SettlementSubmissionOutcome::Timeout,
            Self::Revert(_) => SettlementSubmissionOutcome::Revert,
            Self::Other(_) => SettlementSubmissionOutcome::Failure,
        }
    }

    /// Convert this submission error into an `anyhow::Error`.
    ///
    /// This is implemented as a method instead of `From`/`Into` to avoid any
    /// multiple trait implementation issues because of the `anyhow` blanket
    /// `impl<T: Display> From<T> for anyhow::Error`.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            SubmissionError::Timeout => anyhow!("transaction did not get mined in time"),
            SubmissionError::Revert(Some(message)) => {
                anyhow!("transaction reverted with message {}", message)
            }
            SubmissionError::Revert(None) => anyhow!("transaction reverted"),
            SubmissionError::Other(err) => err,
        }
    }
}

impl From<anyhow::Error> for SubmissionError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

impl From<MethodError> for SubmissionError {
    fn from(err: MethodError) -> Self {
        match err.inner {
            ExecutionError::ConfirmTimeout(_) => SubmissionError::Timeout,
            ExecutionError::Failure(_) | ExecutionError::InvalidOpcode => {
                SubmissionError::Revert(None)
            }
            ExecutionError::Revert(message) => SubmissionError::Revert(message),
            _ => SubmissionError::Other(
                anyhow::Error::from(err).context("settlement transaction failed"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::H256;

    impl PartialEq for SubmissionError {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Revert(left), Self::Revert(right)) => left == right,
                _ => std::mem::discriminant(self) == std::mem::discriminant(other),
            }
        }
    }

    #[test]
    fn converts_method_errors() {
        for (from, to) in [
            (
                ExecutionError::Failure(Default::default()),
                SubmissionError::Revert(None),
            ),
            (ExecutionError::InvalidOpcode, SubmissionError::Revert(None)),
            (
                ExecutionError::Revert(Some("foo".to_owned())),
                SubmissionError::Revert(Some("foo".to_owned())),
            ),
            (
                ExecutionError::ConfirmTimeout(Box::new(
                    ethcontract::transaction::TransactionResult::Hash(H256::default()),
                )),
                SubmissionError::Timeout,
            ),
            (
                ExecutionError::NoLocalAccounts,
                SubmissionError::Other(anyhow!("_")),
            ),
        ] {
            assert_eq!(
                SubmissionError::from(MethodError::from_parts("foo()".to_owned(), from)),
                to,
            )
        }
    }
}
