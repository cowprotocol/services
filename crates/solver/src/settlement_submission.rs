mod dry_run;
pub mod submitter;

use crate::{
    metrics::SettlementSubmissionOutcome, settlement::Settlement,
    settlement_access_list::AccessListEstimating,
};
use anyhow::{anyhow, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    errors::{ExecutionError, MethodError},
    Account, Address, TransactionHash,
};
use futures::FutureExt;
use gas_estimation::{EstimatedGasPrice, GasPriceEstimating};
use primitive_types::{H256, U256};
use shared::Web3;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use submitter::{
    DisabledReason, Strategy, Submitter, SubmitterGasPriceEstimator, SubmitterParams,
    TransactionHandle, TransactionSubmitting,
};
use web3::types::TransactionReceipt;

const ESTIMATE_GAS_LIMIT_FACTOR: f64 = 1.2;

#[derive(Default, Clone)]
pub struct WrappedTxPool(Arc<Mutex<TxPool>>);

impl WrappedTxPool {
    pub fn get(
        &self,
        strategy: Strategy,
        sender: Address,
        nonce: U256,
    ) -> Option<Vec<(TransactionHandle, EstimatedGasPrice)>> {
        self.0
            .lock()
            .unwrap()
            .get_sub_pool(strategy)
            .get(&(sender, nonce))
            .cloned()
    }

    pub fn remove_if(&self, strategy: Strategy, nonce: U256) {
        self.0
            .lock()
            .unwrap()
            .get_sub_pool_mut(strategy)
            .retain(|key, _| key.1 >= nonce);
    }

    pub fn update(
        &self,
        strategy: Strategy,
        sender: Address,
        nonce: U256,
        transactions: Vec<(TransactionHandle, EstimatedGasPrice)>,
    ) {
        self.0
            .lock()
            .unwrap()
            .get_sub_pool_mut(strategy)
            .insert((sender, nonce), transactions);
    }
}

// Key (Address, U256) represents pair (sender, nonce)
type SubTxPool = HashMap<(Address, U256), Vec<(TransactionHandle, EstimatedGasPrice)>>;

#[derive(Default)]
pub struct TxPool {
    eden: SubTxPool,
    flashbots: SubTxPool,
    custom_nodes: SubTxPool,
}

impl TxPool {
    pub fn get_sub_pool(&self, strategy: Strategy) -> &SubTxPool {
        match strategy {
            Strategy::Eden => &self.eden,
            Strategy::Flashbots => &self.flashbots,
            Strategy::CustomNodes => &self.custom_nodes,
        }
    }

    pub fn get_sub_pool_mut(&mut self, strategy: Strategy) -> &mut SubTxPool {
        match strategy {
            Strategy::Eden => &mut self.eden,
            Strategy::Flashbots => &mut self.flashbots,
            Strategy::CustomNodes => &mut self.custom_nodes,
        }
    }
}

pub struct SolutionSubmitter {
    pub web3: Web3,
    pub contract: GPv2Settlement,
    pub gas_price_estimator: Arc<dyn GasPriceEstimating>,
    pub access_list_estimator: Arc<dyn AccessListEstimating>,
    // for gas price estimation
    pub target_confirm_time: Duration,
    pub max_confirm_time: Duration,
    pub retry_interval: Duration,
    pub gas_price_cap: f64,
    pub transaction_strategies: Vec<TransactionStrategy>,
    pub submitted_transactions: WrappedTxPool,
}

pub struct StrategyArgs {
    pub submit_api: Box<dyn TransactionSubmitting>,
    pub max_additional_tip: f64,
    pub additional_tip_percentage_of_max_fee: f64,
}
pub enum TransactionStrategy {
    Eden(StrategyArgs),
    Flashbots(StrategyArgs),
    CustomNodes(StrategyArgs),
    DryRun,
}

impl TransactionStrategy {
    pub fn strategy_args(&self) -> Option<&StrategyArgs> {
        match &self {
            TransactionStrategy::Eden(args) => Some(args),
            TransactionStrategy::Flashbots(args) => Some(args),
            TransactionStrategy::CustomNodes(args) => Some(args),
            TransactionStrategy::DryRun => None,
        }
    }
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
        let is_dry_run: bool = self
            .transaction_strategies
            .iter()
            .any(|strategy| matches!(strategy, TransactionStrategy::DryRun));

        let network_id = self.web3.net().version().await?;

        if is_dry_run {
            Ok(dry_run::log_settlement(account, &self.contract, settlement).await?)
        } else {
            let mut futures = self
                .transaction_strategies
                .iter()
                .map(|strategy| {
                    async {
                        match &*strategy {
                            TransactionStrategy::Eden(_) | TransactionStrategy::Flashbots(_) => {
                                if !matches!(account, Account::Offline(..)) {
                                    return Err(SubmissionError::from(anyhow!(
                                        "Submission to private network requires offline account for signing"
                                    )));
                                }
                            }
                            TransactionStrategy::CustomNodes(_) => {}
                            TransactionStrategy::DryRun => unreachable!(),
                        };

                        let strategy_args = strategy.strategy_args().expect("unreachable code executed");
                        let params = SubmitterParams {
                            target_confirm_time: self.target_confirm_time,
                            gas_estimate,
                            deadline: Some(Instant::now() + self.max_confirm_time),
                            retry_interval: self.retry_interval,
                            network_id: network_id.clone(),
                        };
                        let gas_price_estimator = SubmitterGasPriceEstimator {
                            inner: self.gas_price_estimator.as_ref(),
                            gas_price_cap: self.gas_price_cap,
                            additional_tip_percentage_of_max_fee: Some(strategy_args.additional_tip_percentage_of_max_fee),
                            max_additional_tip: Some(strategy_args.max_additional_tip),
                        };
                        let submitter = Submitter::new(
                            &self.contract,
                            &account,
                            strategy_args.submit_api.as_ref(),
                            &gas_price_estimator,
                            self.access_list_estimator.as_ref(),
                            self.submitted_transactions.clone(),
                        )?;
                        submitter.submit(settlement.clone(), params).await
                    }
                    .boxed()
                })
                .collect::<Vec<_>>();

            loop {
                let (result, _index, rest) = futures::future::select_all(futures).await;
                match result {
                    Ok(receipt) => return Ok(receipt),
                    Err(err) if rest.is_empty() || err.is_transaction_mined() => {
                        return Err(err);
                    }
                    Err(_) => {
                        futures = rest;
                    }
                }
            }
        }
    }
}

/// An error during settlement submission.
#[derive(Debug)]
pub enum SubmissionError {
    /// The transaction reverted in the simulation stage.
    SimulationRevert(Option<String>),
    /// Transaction successfully mined but reverted
    Revert(TransactionHash),
    /// The settlement submission timed out.
    Timeout,
    /// Canceled after revert or timeout
    Canceled(TransactionHash),
    /// The submission is disabled
    Disabled(DisabledReason),
    /// An error occured.
    Other(anyhow::Error),
}

impl SubmissionError {
    /// Returns the outcome for use with metrics.
    pub fn as_outcome(&self) -> SettlementSubmissionOutcome {
        match self {
            Self::SimulationRevert(_) => SettlementSubmissionOutcome::SimulationRevert,
            Self::Timeout => SettlementSubmissionOutcome::Timeout,
            Self::Revert(_) => SettlementSubmissionOutcome::Revert,
            Self::Canceled(_) => SettlementSubmissionOutcome::Cancel,
            Self::Disabled(_) => SettlementSubmissionOutcome::Disabled,
            Self::Other(_) => SettlementSubmissionOutcome::Failed,
        }
    }

    pub fn transaction_hash(&self) -> Option<H256> {
        match self {
            Self::SimulationRevert(_) => None,
            Self::Timeout => None,
            Self::Revert(hash) => Some(*hash),
            Self::Canceled(hash) => Some(*hash),
            Self::Disabled(_) => None,
            Self::Other(_) => None,
        }
    }

    /// Convert this submission error into an `anyhow::Error`.
    ///
    /// This is implemented as a method instead of `From`/`Into` to avoid any
    /// multiple trait implementation issues because of the `anyhow` blanket
    /// `impl<T: Display> From<T> for anyhow::Error`.
    pub fn into_anyhow(self) -> anyhow::Error {
        match self {
            SubmissionError::Revert(hash) => anyhow!("transaction reverted, hash: {:?}", hash),
            SubmissionError::Timeout => anyhow!("transaction did not get mined in time"),
            SubmissionError::SimulationRevert(Some(message)) => {
                anyhow!("transaction simulation reverted with message {}", message)
            }
            SubmissionError::Canceled(hash) => {
                anyhow!(
                    "transaction cancelled after revert or timeout, hash: {:?}",
                    hash
                )
            }
            SubmissionError::SimulationRevert(None) => anyhow!("transaction simulation reverted"),
            SubmissionError::Disabled(reason) => {
                anyhow!("transaction disabled, reason: {:?}", reason)
            }
            SubmissionError::Other(err) => err,
        }
    }

    pub fn is_transaction_mined(&self) -> bool {
        match self {
            SubmissionError::SimulationRevert(_) => false,
            SubmissionError::Revert(_) => true,
            SubmissionError::Timeout => false,
            SubmissionError::Canceled(_) => true,
            SubmissionError::Other(_) => false,
            SubmissionError::Disabled(_) => false,
        }
    }
}

impl From<web3::Error> for SubmissionError {
    fn from(err: web3::Error) -> Self {
        Self::Other(err.into())
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
                SubmissionError::SimulationRevert(None)
            }
            ExecutionError::Revert(message) => SubmissionError::SimulationRevert(message),
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
    use submitter::MockTransactionSubmitting;

    impl PartialEq for SubmissionError {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::SimulationRevert(left), Self::SimulationRevert(right)) => left == right,
                _ => std::mem::discriminant(self) == std::mem::discriminant(other),
            }
        }
    }

    impl Default for StrategyArgs {
        fn default() -> Self {
            Self {
                submit_api: Box::new(MockTransactionSubmitting::new()),
                max_additional_tip: Default::default(),
                additional_tip_percentage_of_max_fee: Default::default(),
            }
        }
    }

    #[test]
    fn converts_method_errors() {
        for (from, to) in [
            (
                ExecutionError::Failure(Default::default()),
                SubmissionError::SimulationRevert(None),
            ),
            (
                ExecutionError::InvalidOpcode,
                SubmissionError::SimulationRevert(None),
            ),
            (
                ExecutionError::Revert(Some("foo".to_owned())),
                SubmissionError::SimulationRevert(Some("foo".to_owned())),
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

    #[test]
    fn transaction_strategy_test() {
        let strategy = TransactionStrategy::Eden(StrategyArgs::default());
        assert!(strategy.strategy_args().is_some());

        let strategy = TransactionStrategy::Flashbots(StrategyArgs::default());
        assert!(strategy.strategy_args().is_some());

        let strategy = TransactionStrategy::CustomNodes(StrategyArgs::default());
        assert!(strategy.strategy_args().is_some());

        let strategy = TransactionStrategy::DryRun;
        assert!(strategy.strategy_args().is_none());
    }

    #[test]
    fn wrapped_tx_pool() {
        let strategy = Strategy::Eden;
        let sender = Address::default();
        let nonce = U256::zero();
        let transactions: Vec<(TransactionHandle, EstimatedGasPrice)> = Default::default();

        let submitted_transactions = WrappedTxPool::default();

        submitted_transactions.update(strategy, sender, nonce, transactions);
        let entry = submitted_transactions.get(strategy, sender, nonce);
        assert!(entry.is_some());

        submitted_transactions.remove_if(strategy, 0.into());
        let entry = submitted_transactions.get(strategy, sender, nonce);
        assert!(entry.is_some());

        submitted_transactions.remove_if(strategy, 1.into());
        let entry = submitted_transactions.get(strategy, sender, nonce);
        assert!(entry.is_none());
    }
}
