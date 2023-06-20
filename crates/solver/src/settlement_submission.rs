mod dry_run;
pub mod gelato;
pub mod submitter;

use {
    self::gelato::GelatoSubmitter,
    crate::{
        metrics::SettlementSubmissionOutcome,
        settlement::{self, Revertable, Settlement},
        settlement_access_list::AccessListEstimating,
    },
    anyhow::{anyhow, Result},
    contracts::GPv2Settlement,
    ethcontract::{
        errors::{ExecutionError, MethodError},
        Account,
        Address,
        TransactionHash,
    },
    futures::FutureExt,
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    primitive_types::{H256, U256},
    shared::{
        code_fetching::CodeFetching,
        encoded_settlement::EncodedSettlement,
        ethrpc::Web3,
        http_solver::model::{InternalizationStrategy, SubmissionPreference},
    },
    std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    },
    submitter::{
        DisabledReason,
        Strategy,
        Submitter,
        SubmitterGasPriceEstimator,
        SubmitterParams,
        TransactionHandle,
        TransactionSubmitting,
    },
    tracing::Instrument,
    web3::types::TransactionReceipt,
};

/// Computes a gas limit from a gas estimate that accounts for some buffer in
/// case racing state changes result in slightly more heavy computation at
/// execution time.
pub fn gas_limit_for_estimate(gas_estimate: U256) -> U256 {
    const ESTIMATE_GAS_LIMIT_FACTOR: f64 = 1.2;
    U256::from_f64_lossy(gas_estimate.to_f64_lossy() * ESTIMATE_GAS_LIMIT_FACTOR)
}

#[derive(Debug)]
pub struct SubTxPool {
    pub strategy: Strategy,
    // Key (Address, U256) represents pair (sender, nonce)
    pub pools: HashMap<(Address, U256), Vec<(TransactionHandle, GasPrice1559)>>,
}
type TxPool = Arc<Mutex<Vec<SubTxPool>>>;

#[derive(Debug, Default, Clone)]
pub struct GlobalTxPool {
    pub pools: TxPool,
}

impl GlobalTxPool {
    pub fn add_sub_pool(&self, strategy: Strategy) -> SubTxPoolRef {
        let pools = self.pools.clone();
        let index = {
            let mut pools = pools.lock().unwrap();
            let index = pools.len();
            pools.push(SubTxPool {
                strategy,
                pools: Default::default(),
            });
            index
        };
        SubTxPoolRef { pools, index }
    }
}

/// Currently used to access only specific sub tx pool (indexed one) in the list
/// of pools. Can be used to access other sub tx pools if needed.
#[derive(Default, Clone)]
pub struct SubTxPoolRef {
    pools: TxPool,
    index: usize,
}

impl SubTxPoolRef {
    pub fn get(
        &self,
        sender: Address,
        nonce: U256,
    ) -> Option<Vec<(TransactionHandle, GasPrice1559)>> {
        self.pools.lock().unwrap()[self.index]
            .pools
            .get(&(sender, nonce))
            .cloned()
    }

    /// Remove old transactions with too low nonce
    pub fn remove_older_than(&self, nonce: U256) {
        self.pools.lock().unwrap()[self.index]
            .pools
            .retain(|key, _| key.1 >= nonce);
    }

    pub fn update(
        &self,
        sender: Address,
        nonce: U256,
        transactions: Vec<(TransactionHandle, GasPrice1559)>,
    ) {
        self.pools.lock().unwrap()[self.index]
            .pools
            .insert((sender, nonce), transactions);
    }

    pub fn clear(&self) {
        self.pools.lock().unwrap()[self.index].pools.clear();
    }
}

pub struct SubmissionReceipt {
    pub tx: TransactionReceipt,
    /// Strategy used for the mined transaction. Needed for metric purposes.
    pub strategy: &'static str,
}

impl From<SubmissionReceipt> for TransactionReceipt {
    fn from(value: SubmissionReceipt) -> Self {
        value.tx
    }
}

pub struct SolutionSubmitter {
    pub web3: Web3,
    pub settlement: GPv2Settlement,
    pub settlement_encoding_contracts: settlement::Contracts,
    pub gas_price_estimator: Arc<dyn GasPriceEstimating>,
    pub access_list_estimator: Arc<dyn AccessListEstimating>,
    // for gas price estimation
    pub target_confirm_time: Duration,
    pub max_confirm_time: Duration,
    pub retry_interval: Duration,
    pub transaction_strategies: Vec<TransactionStrategy>,
    pub code_fetcher: Arc<dyn CodeFetching>,
}

pub struct StrategyArgs {
    pub submit_api: Box<dyn TransactionSubmitting>,
    pub max_additional_tip: f64,
    pub additional_tip_percentage_of_max_fee: f64,
    pub sub_tx_pool: SubTxPoolRef,
    pub use_soft_cancellations: bool,
}

pub enum TransactionStrategy {
    Eden(StrategyArgs),
    Flashbots(StrategyArgs),
    PublicMempool(StrategyArgs),
    Gelato(Arc<GelatoSubmitter>),
    DryRun,
}

impl TransactionStrategy {
    pub fn strategy_args(&self) -> Option<&StrategyArgs> {
        match &self {
            TransactionStrategy::Eden(args) => Some(args),
            TransactionStrategy::Flashbots(args) => Some(args),
            TransactionStrategy::PublicMempool(args) => Some(args),
            TransactionStrategy::Gelato(_) | TransactionStrategy::DryRun => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match &self {
            TransactionStrategy::Eden(_) => "Eden",
            TransactionStrategy::Flashbots(_) => "Flashbots",
            TransactionStrategy::PublicMempool(_) => "Mempool",
            TransactionStrategy::Gelato(_) => "Gelato",
            TransactionStrategy::DryRun => "DryRun",
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
        max_fee_per_gas: f64,
        account: Account,
        nonce: U256,
    ) -> Result<SubmissionReceipt, SubmissionError> {
        let settlement = SubmitterSettlement::new(settlement, &self.settlement_encoding_contracts);

        // Other transaction strategies than the ones below, depend on an
        // account signing a raw transaction for a nonce, and waiting until that
        // nonce increases to detect that it actually mined. However, the
        // strategies below are **not** compatible with this. So if one of them
        // is specified, use it exclusively for submitting and exit the loop.
        // TODO(nlordell): We can refactor the `SolutionSubmitter` interface to
        // better reflect configuration incompatibilities like this.
        for strategy in &self.transaction_strategies {
            match strategy {
                TransactionStrategy::DryRun => {
                    return dry_run::log_settlement(account, &self.settlement, settlement)
                        .await
                        .map(|tx| SubmissionReceipt {
                            tx,
                            strategy: strategy.label(),
                        })
                        .map_err(Into::into);
                }
                TransactionStrategy::Gelato(gelato) => {
                    return tokio::time::timeout(
                        self.max_confirm_time,
                        gelato.relay_settlement(account, settlement.encoded),
                    )
                    .await
                    .map(|tx| {
                        tx.map(|tx| SubmissionReceipt {
                            tx,
                            strategy: strategy.label(),
                        })
                    })
                    .map_err(|_| SubmissionError::Timeout)?;
                }
                _ => {}
            }
        }

        // combine protocol enabled strategies with settlement prefered strategies
        let strategies = match settlement.inner.submitter {
            SubmissionPreference::ProtocolDefault => self.transaction_strategies.as_slice(),
            SubmissionPreference::Flashbots => {
                let position = self
                    .transaction_strategies
                    .iter()
                    .position(|strategy| matches!(strategy, TransactionStrategy::Flashbots(_)));
                match position {
                    Some(index) => &self.transaction_strategies[index..index + 1],
                    // if flashbots are disabled by protocol, default to protocol strategies
                    None => self.transaction_strategies.as_slice(),
                }
            }
            SubmissionPreference::PublicMempool => {
                let position = self
                    .transaction_strategies
                    .iter()
                    .position(|strategy| matches!(strategy, TransactionStrategy::PublicMempool(_)));
                match position {
                    Some(index) => &self.transaction_strategies[index..index + 1],
                    // if public mempool is disabled by protocol, default to protocol strategies
                    None => self.transaction_strategies.as_slice(),
                }
            }
        };

        let network_id = self.web3.net().version().await?;
        let mut futures = strategies
            .iter()
            .enumerate()
            .map(|(i, strategy)| {
                self.settle_with_strategy(
                    strategy,
                    &account,
                    nonce,
                    gas_estimate,
                    max_fee_per_gas,
                    network_id.clone(),
                    settlement.clone(),
                )
                .instrument(tracing::info_span!(
                    "submission",
                    name = %strategy.label(),
                    i
                ))
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

    #[allow(clippy::too_many_arguments)]
    async fn settle_with_strategy(
        &self,
        strategy: &TransactionStrategy,
        account: &Account,
        nonce: U256,
        gas_estimate: U256,
        max_fee_per_gas: f64,
        network_id: String,
        settlement: SubmitterSettlement,
    ) -> Result<SubmissionReceipt, SubmissionError> {
        match strategy {
            TransactionStrategy::Eden(_) | TransactionStrategy::Flashbots(_) => {
                if !matches!(account, Account::Offline(..)) {
                    return Err(SubmissionError::from(anyhow!(
                        "Submission to private network requires offline account for signing"
                    )));
                }
            }
            TransactionStrategy::PublicMempool(_) => {}
            _ => unreachable!(),
        };

        let strategy_args = strategy.strategy_args().expect("unreachable code executed");

        // No extra tip required if there is no revert risk
        let (additional_tip_percentage_of_max_fee, max_additional_tip) =
            if settlement.inner.revertable() == Revertable::NoRisk {
                tracing::debug!("Disabling additional tip because of NoRisk settlement");
                (0., 0.)
            } else {
                (
                    strategy_args.additional_tip_percentage_of_max_fee,
                    strategy_args.max_additional_tip,
                )
            };

        let params = SubmitterParams {
            target_confirm_time: self.target_confirm_time,
            gas_estimate,
            deadline: Some(Instant::now() + self.max_confirm_time),
            retry_interval: self.retry_interval,
            network_id,
            additional_call_data: Default::default(),
            use_soft_cancellations: strategy_args.use_soft_cancellations,
        };
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: self.gas_price_estimator.as_ref(),
            max_fee_per_gas,
            additional_tip_percentage_of_max_fee,
            max_additional_tip,
        };
        let submitter = Submitter::new(
            &self.settlement,
            account,
            nonce,
            strategy_args.submit_api.as_ref(),
            &gas_price_estimator,
            self.access_list_estimator.as_ref(),
            strategy_args.sub_tx_pool.clone(),
            self.web3.clone(),
            self.code_fetcher.as_ref(),
        )?;
        submitter
            .submit(settlement, params)
            .await
            .map(|tx| SubmissionReceipt {
                tx,
                strategy: strategy.label(),
            })
    }
}

/// A settlement that is ready to be used by submitters.
#[derive(Clone, Debug)]
pub struct SubmitterSettlement {
    inner: Settlement,
    encoded: EncodedSettlement,
}

impl SubmitterSettlement {
    pub fn new(settlement: Settlement, contracts: &settlement::Contracts) -> Self {
        Self {
            inner: settlement.clone(),
            encoded: settlement.encode(
                contracts,
                InternalizationStrategy::SkipInternalizableInteraction,
            ),
        }
    }

    #[cfg(test)]
    fn for_settlement(settlement: Settlement) -> Self {
        assert!(!settlement.encoder.has_custom_order_interactions());
        Self::new(settlement, &settlement::Contracts::default())
    }
}

#[cfg(test)]
impl Default for SubmitterSettlement {
    fn default() -> Self {
        Self::for_settlement(Settlement::default())
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
    /// An error occurred.
    Other(anyhow::Error),
}

impl std::fmt::Display for SubmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SubmissionError {}

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

    /// Returns the transaction hash of a reverted on-chain settlement.
    pub fn revert_transaction_hash(&self) -> Option<H256> {
        match self {
            Self::SimulationRevert(_) => None,
            Self::Timeout => None,
            Self::Revert(hash) => Some(*hash),
            Self::Canceled(_) => None,
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
            Self::Revert(hash) => anyhow!("transaction reverted, hash: {:?}", hash),
            Self::Timeout => anyhow!("transaction did not get mined in time"),
            Self::SimulationRevert(Some(message)) => {
                anyhow!("transaction simulation reverted with message {}", message)
            }
            Self::Canceled(hash) => {
                anyhow!(
                    "transaction cancelled after revert or timeout, hash: {:?}",
                    hash
                )
            }
            Self::SimulationRevert(None) => anyhow!("transaction simulation reverted"),
            Self::Disabled(reason) => {
                anyhow!("transaction disabled, reason: {:?}", reason)
            }
            Self::Other(err) => err,
        }
    }

    pub fn is_transaction_mined(&self) -> bool {
        match self {
            Self::SimulationRevert(_) => false,
            Self::Revert(_) => true,
            Self::Timeout => false,
            Self::Canceled(_) => true,
            Self::Other(_) => false,
            Self::Disabled(_) => false,
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
    use {super::*, ethcontract::H256, submitter::MockTransactionSubmitting};

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
                sub_tx_pool: Default::default(),
                use_soft_cancellations: false,
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

        let strategy = TransactionStrategy::PublicMempool(StrategyArgs::default());
        assert!(strategy.strategy_args().is_some());

        let strategy = TransactionStrategy::DryRun;
        assert!(strategy.strategy_args().is_none());
    }

    #[test]
    fn global_tx_pool() {
        let sender = Address::default();
        let nonce = U256::zero();
        let transactions: Vec<(TransactionHandle, GasPrice1559)> = Default::default();

        let submitted_transactions = GlobalTxPool::default().add_sub_pool(Strategy::PublicMempool);

        submitted_transactions.update(sender, nonce, transactions);
        let entry = submitted_transactions.get(sender, nonce);
        assert!(entry.is_some());

        submitted_transactions.remove_older_than(0.into());
        let entry = submitted_transactions.get(sender, nonce);
        assert!(entry.is_some());

        submitted_transactions.remove_older_than(1.into());
        let entry = submitted_transactions.get(sender, nonce);
        assert!(entry.is_none());
    }
}
