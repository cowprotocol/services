// Design:
// As in the traditional transaction submission workflow the main work in this module is checking
// the gas price in a loop and updating the transaction when the gas price increases. This differs
// so that we can make use of the property that flashbots transactions do not cost gas if they fail.
// When we detect that the transaction would no longer succeed we stop trying to submit and return
// so that the solver can run again.
// In addition to simulation failure we make use of a deadline after which submission attempts also
// stop. This allows the solver to update and improve a solution even if it hasn't yet become
// invalid.
// We do not know in advance which of our submitted transactions will get mined. Instead of polling
// all of them we only check the account's nonce as an optimization. When this happens all our
// transactions definitely become invalid (even if the transaction came for whatever reason
// from outside) so it is only at that point that we need to check the hashes individually to the
// find the one that got mined (if any).

mod common;
pub mod custom_nodes_api;
pub mod eden_api;
pub mod flashbots_api;

use super::{SubTxPoolRef, SubmissionError, ESTIMATE_GAS_LIMIT_FACTOR};
use crate::{
    settlement::Settlement, settlement_access_list::AccessListEstimating,
    settlement_simulation::settle_method_builder,
};
use anyhow::{anyhow, ensure, Context, Result};
use contracts::GPv2Settlement;
use ethcontract::{contract::MethodBuilder, transaction::TransactionBuilder, Account};
use futures::FutureExt;
use gas_estimation::{GasPrice1559, GasPriceEstimating};
use primitive_types::{H256, U256};
use shared::{Web3, Web3Transport};
use std::{
    fmt,
    time::{Duration, Instant},
};
use web3::types::{AccessList, TransactionReceipt, U64};

/// Minimal gas price replacement factor
const GAS_PRICE_BUMP: f64 = 1.125;

/// Parameters for transaction submitting
#[derive(Clone, Default)]
pub struct SubmitterParams {
    /// Desired duration to include the transaction in a block
    pub target_confirm_time: Duration, //todo ds change to blocks in the following PR
    /// Estimated gas consumption of a transaction
    pub gas_estimate: U256,
    /// Maximum duration of a single run loop
    pub deadline: Option<Instant>,
    /// Resimulate and resend transaction on every retry_interval seconds
    pub retry_interval: Duration,
    /// Network id (mainnet, rinkeby, goerli, gnosis chain)
    pub network_id: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SubmissionLoopStatus {
    Enabled(AdditionalTip),
    Disabled(DisabledReason),
}

#[derive(Debug, Clone, Copy)]
pub enum Strategy {
    Eden,
    Flashbots,
    CustomNodes,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum AdditionalTip {
    Off,
    On,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DisabledReason {
    MevExtractable,
}

#[derive(Debug, Clone, Copy)]
pub struct TransactionHandle {
    pub handle: H256,
    pub tx_hash: H256,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait TransactionSubmitting: Send + Sync {
    /// Submits transation to the specific network (public mempool, eden, flashbots...).
    /// Returns transaction handle
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle>;
    /// Cancels already submitted transaction using the noop transaction
    async fn cancel_transaction(
        &self,
        tx: TransactionBuilder<Web3Transport>,
    ) -> Result<TransactionHandle>;
    /// Checks if transaction submitting is enabled at the moment
    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus;
    /// Returns type of the submitter.
    fn name(&self) -> Strategy;
}

/// Gas price estimator specialized for sending transactions to the network
#[derive(Clone)]
pub struct SubmitterGasPriceEstimator<'a> {
    pub inner: &'a dyn GasPriceEstimating,
    /// Additionally increase max_priority_fee_per_gas by percentage of max_fee_per_gas, in order to increase the chances of a transaction being mined
    pub additional_tip_percentage_of_max_fee: Option<f64>,
    /// Maximum max_priority_fee_per_gas additional increase
    pub max_additional_tip: Option<f64>,
    /// Maximum max_fee_per_gas to pay for a transaction
    pub gas_price_cap: f64,
    /// Gas price from pending transaction from previous submission loop
    pub pending_gas_price: Option<GasPrice1559>,
}

impl SubmitterGasPriceEstimator<'_> {
    pub fn with_additional_tip(&self, max_additional_tip: Option<f64>) -> Self {
        Self {
            max_additional_tip,
            ..*self
        }
    }
    pub fn with_pending_gas_price(&self, pending_gas_price: Option<GasPrice1559>) -> Self {
        Self {
            pending_gas_price,
            ..*self
        }
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for SubmitterGasPriceEstimator<'_> {
    async fn estimate_with_limits(
        &self,
        gas_limit: f64,
        time_limit: Duration,
    ) -> Result<GasPrice1559> {
        let gas_price = match self.inner.estimate_with_limits(gas_limit, time_limit).await {
            Ok(mut gas_price) if gas_price.max_fee_per_gas <= self.gas_price_cap => {
                // boost miner tip to increase our chances of being included in a block
                gas_price.max_priority_fee_per_gas +=
                    self.max_additional_tip.unwrap_or_default().min(
                        gas_price.max_fee_per_gas
                            * self
                                .additional_tip_percentage_of_max_fee
                                .unwrap_or_default(),
                    );
                Ok(gas_price)
            }
            Ok(gas_price) => Err(anyhow!(
                "gas station gas price {} is larger than cap {}",
                gas_price.max_fee_per_gas,
                self.gas_price_cap
            )),
            Err(err) => Err(err),
        };

        // If pending gas price exist, return max(gas_price, pending_gas_price*1.125)
        gas_price.map(|gas_price| match self.pending_gas_price {
            Some(pending_gas_price) => {
                tracing::debug!("found pending transaction: {:?}", pending_gas_price);
                let pending_gas_price = pending_gas_price.bump(GAS_PRICE_BUMP).ceil();
                if gas_price.max_fee_per_gas >= pending_gas_price.max_fee_per_gas
                    && gas_price.max_priority_fee_per_gas
                        >= pending_gas_price.max_priority_fee_per_gas
                {
                    gas_price
                } else {
                    pending_gas_price
                }
            }
            None => gas_price,
        })
    }
}

pub struct Submitter<'a> {
    contract: &'a GPv2Settlement,
    account: &'a Account,
    submit_api: &'a dyn TransactionSubmitting,
    gas_price_estimator: &'a SubmitterGasPriceEstimator<'a>,
    access_list_estimator: &'a dyn AccessListEstimating,
    submitted_transactions: SubTxPoolRef,
}

impl<'a> Submitter<'a> {
    pub fn new(
        contract: &'a GPv2Settlement,
        account: &'a Account,
        submit_api: &'a dyn TransactionSubmitting,
        gas_price_estimator: &'a SubmitterGasPriceEstimator<'a>,
        access_list_estimator: &'a dyn AccessListEstimating,
        submitted_transactions: SubTxPoolRef,
    ) -> Result<Self> {
        Ok(Self {
            contract,
            account,
            submit_api,
            gas_price_estimator,
            access_list_estimator,
            submitted_transactions,
        })
    }
}

impl<'a> Submitter<'a> {
    /// Submit a settlement to the contract, updating the transaction with gas prices if they increase.
    ///
    /// Only works on mainnet.
    pub async fn submit(
        &self,
        settlement: Settlement,
        params: SubmitterParams,
    ) -> Result<TransactionReceipt, SubmissionError> {
        let nonce = self.nonce().await?;
        let name = self.submit_api.name();

        tracing::debug!(
            "starting solution submission at nonce {} with submitter {}",
            nonce,
            name
        );

        self.submitted_transactions.remove_older_than(nonce);

        // Take pending transactions from previous submission loops with the same nonce
        // Those exist if
        // 1. Previous loop timed out and no transaction was mined
        // 2. Previous loop ended with simulation revert, and cancellation tx was sent but not mined
        let mut transactions = self
            .submitted_transactions
            .get(self.account.address(), nonce)
            .unwrap_or_default();

        // Continually simulate and submit transactions
        let submit_future = self.submit_with_increasing_gas_prices_until_simulation_fails(
            settlement,
            nonce,
            &params,
            &mut transactions,
        );

        // Nonce future is used to detect if tx is mined
        let nonce_future = self.wait_for_nonce_to_change(nonce);

        // If specified, deadline future stops submitting when deadline is reached
        let deadline_future = tokio::time::sleep(match params.deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => Duration::from_secs(u64::MAX),
        });

        let fallback_result = tokio::select! {
            method_error = submit_future.fuse() => {
                tracing::debug!("stopping submission for {} because simulation failed: {:?}", name, method_error);
                Err(method_error)
            },
            new_nonce = nonce_future.fuse() => {
                tracing::debug!("stopping submission for {} because account nonce changed to {}", name, new_nonce);
                Ok(None)
            },
            _ = deadline_future.fuse() => {
                tracing::debug!("stopping submission for {} because deadline has been reached. cancelling last submitted transaction...", name);

                if let Some((_, gas_price)) = transactions.last() {
                    let gas_price = gas_price.bump(GAS_PRICE_BUMP).ceil();
                    match self
                        .cancel_transaction(&gas_price, nonce)
                        .await
                    {
                        Ok(handle) => transactions.push((handle, gas_price)),
                        Err(err) => tracing::warn!("cancellation failed: {:?}", err),
                    }
                }
                Ok(None)
            },
        };

        // After stopping submission of new transactions we wait for some time to give a potentially
        // mined previously submitted transaction time to propagate to our node.

        // Example:
        // 1. We submit tx to ethereum node, and we start counting 10s pause before new loop iteration.
        // 2. In the meantime, block A gets mined somewhere in the network (not containing our tx)
        // 3. After some time block B is mined somewhere in the network (containing our tx)
        // 4. Our node receives block A.
        // 5. Our 10s is up but our node received only block A because of the delay in block propagation. We simulate tx and it fails, we return back
        // 6. If we don't wait another 20s to receive block B, we wont see mined tx.

        if !transactions.is_empty() {
            // Update (overwrite) the submitted transaction list with `transactions` variable that,
            // at this point, contains both transactions from previous submission loop and
            // transactions from current submission loop
            self.submitted_transactions
                .update(self.account.address(), nonce, transactions.clone());

            const MINED_TX_PROPAGATE_TIME: Duration = Duration::from_secs(20);
            const MINED_TX_CHECK_INTERVAL: Duration = Duration::from_secs(5);
            let tx_to_propagate_deadline = Instant::now() + MINED_TX_PROPAGATE_TIME;

            tracing::debug!(
                "waiting up to {} seconds for {} to see if a transaction was mined",
                MINED_TX_PROPAGATE_TIME.as_secs(),
                name
            );

            let transactions = transactions
                .into_iter()
                .map(|(handle, _)| handle.tx_hash)
                .collect::<Vec<_>>();

            loop {
                if let Some(receipt) =
                    find_mined_transaction(&self.contract.raw_instance().web3(), &transactions)
                        .await
                {
                    tracing::debug!("{} found mined transaction {:?}", name, receipt);
                    track_mined_transactions(&format!("{name}"));
                    return status(receipt);
                }
                if Instant::now() + MINED_TX_CHECK_INTERVAL > tx_to_propagate_deadline {
                    break;
                }
                tokio::time::sleep(MINED_TX_CHECK_INTERVAL).await;
            }
        }

        tracing::debug!("{} did not find any mined transaction", name);
        fallback_result
            .transpose()
            .unwrap_or(Err(SubmissionError::Timeout))
    }

    async fn nonce(&self) -> Result<U256> {
        self.contract
            .raw_instance()
            .web3()
            .eth()
            .transaction_count(self.account.address(), None)
            .await
            .context("transaction_count")
    }

    /// Keep polling the account's nonce until it is different from initial_nonce returning the new
    /// nonce.
    async fn wait_for_nonce_to_change(&self, initial_nonce: U256) -> U256 {
        const POLL_INTERVAL: Duration = Duration::from_secs(1);
        loop {
            match self.nonce().await {
                Ok(nonce) if nonce != initial_nonce => return nonce,
                Ok(_) => (),
                Err(err) => tracing::error!("web3 error while getting nonce: {:?}", err),
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Keep submitting the settlement transaction to the network as gas price changes.
    ///
    /// Returns when simulation of the transaction fails. This likely happens if the settlement
    /// becomes invalid due to changing prices or the account's nonce changes.
    ///
    /// Potential transaction hashes are communicated back through a shared vector.
    async fn submit_with_increasing_gas_prices_until_simulation_fails(
        &self,
        settlement: Settlement,
        nonce: U256,
        params: &SubmitterParams,
        transactions: &mut Vec<(TransactionHandle, GasPrice1559)>,
    ) -> SubmissionError {
        let submitter_name = self.submit_api.name();
        let target_confirm_time = Instant::now() + params.target_confirm_time;

        tracing::debug!(
            "submit_with_increasing_gas_prices_until_simulation_fails entered with submitter: {}",
            submitter_name
        );

        // Try to find submitted transaction from previous submission loop (with the same address and nonce)
        let mut pending_gas_price = transactions.last().cloned().map(|(_, gas_price)| gas_price);

        let mut access_list: Option<AccessList> = None;

        loop {
            tracing::debug!("entered loop with submitter: {}", submitter_name);

            let submission_status = self
                .submit_api
                .submission_status(&settlement, &params.network_id);
            let estimator = match submission_status {
                SubmissionLoopStatus::Disabled(reason) => {
                    tracing::debug!(
                        "strategy {} temporarily disabled, reason: {:?}",
                        submitter_name,
                        reason
                    );
                    return SubmissionError::from(anyhow!("strategy temporarily disabled"));
                }
                SubmissionLoopStatus::Enabled(AdditionalTip::Off) => self
                    .gas_price_estimator
                    .with_additional_tip(None)
                    .with_pending_gas_price(pending_gas_price),
                SubmissionLoopStatus::Enabled(AdditionalTip::On) => self
                    .gas_price_estimator
                    .with_pending_gas_price(pending_gas_price),
            };
            pending_gas_price = None;
            // Account for some buffer in the gas limit in case racing state changes result in slightly more heavy computation at execution time.
            let gas_limit = params.gas_estimate.to_f64_lossy() * ESTIMATE_GAS_LIMIT_FACTOR;
            let time_limit = target_confirm_time.saturating_duration_since(Instant::now());
            let gas_price = match estimator.estimate_with_limits(gas_limit, time_limit).await {
                Ok(gas_price) => gas_price,
                Err(err) => {
                    tracing::error!("gas estimation failed: {:?}", err);
                    tokio::time::sleep(params.retry_interval).await;
                    continue;
                }
            };

            // create transaction

            let method = self
                .build_method(settlement.clone(), &gas_price, nonce, gas_limit)
                .await;

            // append access list

            let method = match access_list.as_ref() {
                Some(access_list) => method.access_list(access_list.clone()),
                None => match self.estimate_access_list(&method.tx).await {
                    Ok(new_access_list) => {
                        access_list = Some(new_access_list.clone());
                        method.access_list(new_access_list)
                    }
                    Err(err) => {
                        tracing::debug!("access list not created, reason: {:?}", err);
                        method
                    }
                },
            };

            // simulate transaction

            if let Err(err) = method.clone().view().call().await {
                if let Some((_, previous_gas_price)) = transactions.last() {
                    let gas_price = previous_gas_price.bump(GAS_PRICE_BUMP).ceil();
                    match self.cancel_transaction(&gas_price, nonce).await {
                        Ok(handle) => transactions.push((handle, gas_price)),
                        Err(err) => tracing::warn!("cancellation failed: {:?}", err),
                    }
                }
                return SubmissionError::from(err);
            }

            // if gas price has not increased enough, skip submitting the transaction.
            if let Some(previous_gas_price) = transactions
                .last()
                .map(|(_, previous_gas_price)| previous_gas_price)
            {
                let previous_gas_price = previous_gas_price.bump(GAS_PRICE_BUMP).ceil();
                if gas_price.max_priority_fee_per_gas < previous_gas_price.max_priority_fee_per_gas
                    || gas_price.max_fee_per_gas < previous_gas_price.max_fee_per_gas
                {
                    tokio::time::sleep(params.retry_interval).await;
                    continue;
                }
            }

            tracing::debug!(
                "creating transaction with gas price (base_fee={}, max_fee={}, tip={}), gas estimate {}, submitter name: {}",
                gas_price.base_fee_per_gas,
                gas_price.max_fee_per_gas,
                gas_price.max_priority_fee_per_gas,
                params.gas_estimate,
                submitter_name,
            );

            // execute transaction

            match self.submit_api.submit_transaction(method.tx).await {
                Ok(handle) => {
                    tracing::debug!(
                        submitter = %submitter_name, ?handle,
                        "submitted transaction",
                    );
                    transactions.push((handle, gas_price));
                }
                Err(err) => {
                    tracing::warn!(
                        submitter = %submitter_name, ?err,
                        "submission failed",
                    );
                }
            }
            tokio::time::sleep(params.retry_interval).await;
        }
    }

    /// Prepare transaction for simulation
    async fn build_method(
        &self,
        settlement: Settlement,
        gas_price: &GasPrice1559,
        nonce: U256,
        gas_limit: f64,
    ) -> MethodBuilder<Web3Transport, ()> {
        settle_method_builder(self.contract, settlement.into(), self.account.clone())
            .nonce(nonce)
            .gas(U256::from_f64_lossy(gas_limit))
            .gas_price(crate::into_gas_price(gas_price))
    }

    /// Estimate access list and validate
    async fn estimate_access_list(
        &self,
        tx: &TransactionBuilder<Web3Transport>,
    ) -> Result<AccessList> {
        let access_list = self.access_list_estimator.estimate_access_list(tx).await?;
        let (gas_before_access_list, gas_after_access_list) = futures::try_join!(
            tx.clone().estimate_gas(),
            tx.clone().access_list(access_list.clone()).estimate_gas()
        )?;

        ensure!(
            gas_before_access_list > gas_after_access_list,
            "access list exist but does not lower the gas usage"
        );
        let gas_percent_saved = (gas_before_access_list.to_f64_lossy()
            - gas_after_access_list.to_f64_lossy())
            / gas_before_access_list.to_f64_lossy()
            * 100.;
        tracing::debug!(
            "gas before/after access list: {}/{}, access_list: {:?}, gas percent saved: {}",
            gas_before_access_list,
            gas_after_access_list,
            access_list,
            gas_percent_saved
        );
        Ok(access_list)
    }

    /// Prepare noop transaction. This transaction does transfer of 0 value to self and always spends 21000 gas.
    fn build_noop_transaction(
        &self,
        gas_price: &GasPrice1559,
        nonce: U256,
    ) -> TransactionBuilder<Web3Transport> {
        TransactionBuilder::new(self.contract.raw_instance().web3())
            .from(self.account.clone())
            .to(self.account.address())
            .nonce(nonce)
            .gas_price(crate::into_gas_price(gas_price))
            .gas(21000.into())
    }

    /// Prepare all data needed for cancellation of previously submitted transaction and execute cancellation
    async fn cancel_transaction(
        &self,
        gas_price: &GasPrice1559,
        nonce: U256,
    ) -> Result<TransactionHandle> {
        let noop_transaction = self.build_noop_transaction(gas_price, nonce);
        self.submit_api.cancel_transaction(noop_transaction).await
    }
}

fn status(receipt: TransactionReceipt) -> Result<TransactionReceipt, SubmissionError> {
    if let Some(status) = receipt.status {
        if status == U64::zero() {
            // failing transaction
            return Err(SubmissionError::Revert(receipt.transaction_hash));
        } else if status == U64::one() && receipt.from == receipt.to.unwrap_or_default() {
            // noop transaction
            return Err(SubmissionError::Canceled(receipt.transaction_hash));
        }
    }
    // successfull transaction
    Ok(receipt)
}

/// From a list of potential hashes find one that was mined.
async fn find_mined_transaction(web3: &Web3, hashes: &[H256]) -> Option<TransactionReceipt> {
    // It would be nice to use the nonce and account address to find the transaction hash but there
    // is no way to do this in ethrpc api so we have to check the candidates one by one.
    let web3 = web3::Web3::new(web3::transports::Batch::new(web3.transport()));
    let futures = hashes
        .iter()
        .map(|&hash| web3.eth().transaction_receipt(hash))
        .collect::<Vec<_>>();
    if let Err(err) = web3.transport().submit_batch().await {
        tracing::error!("mined transaction batch failed: {:?}", err);
        return None;
    }
    for future in futures {
        match future.now_or_never().unwrap() {
            Err(err) => {
                tracing::error!("mined transaction individual failed: {:?}", err);
            }
            Ok(Some(transaction)) if transaction.block_hash.is_some() => return Some(transaction),
            Ok(_) => (),
        }
    }
    None
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "submission_strategies")]
struct Metrics {
    /// Tracks how many transactions get successfully submitted with the different submission strategies.
    #[metric(labels("submitter", "result"))]
    submissions: prometheus::IntCounterVec,
    /// Tracks how many transactions get successfully mined by the different submission strategies.
    #[metric(labels("submitter"))]
    mined_transactions: prometheus::IntCounterVec,
}

pub(crate) fn track_submission_success(submitter: &str, was_successful: bool) {
    let result = if was_successful { "success" } else { "error" };
    Metrics::instance(global_metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
        .submissions
        .with_label_values(&[submitter, result])
        .inc();
}

fn track_mined_transactions(submitter: &str) {
    Metrics::instance(global_metrics::get_metric_storage_registry())
        .expect("unexpected error getting metrics instance")
        .mined_transactions
        .with_label_values(&[submitter])
        .inc();
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use crate::settlement_access_list::{create_priority_estimator, AccessListEstimatorType};

    use super::super::submitter::flashbots_api::FlashbotsApi;
    use super::*;
    use ethcontract::PrivateKey;
    use gas_estimation::blocknative::BlockNative;
    use reqwest::Client;
    use shared::gas_price_estimation::FakeGasPriceEstimator;
    use shared::transport::create_env_test_transport;
    use tracing::level_filters::LevelFilter;

    #[tokio::test]
    #[ignore]
    async fn flashbots_mainnet_settlement() {
        shared::tracing::initialize(
            "solver=debug,shared=debug,shared::transport::http=info",
            LevelFilter::OFF,
        );

        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        assert_eq!(chain_id, 1);
        let private_key: PrivateKey = std::env::var("PRIVATE_KEY").unwrap().parse().unwrap();
        let account = Account::Offline(private_key, Some(chain_id));
        let contract = crate::get_settlement_contract(&web3).await.unwrap();
        let flashbots_api = FlashbotsApi::new(Client::new(), "https://rpc.flashbots.net").unwrap();
        let mut header = reqwest::header::HeaderMap::new();
        header.insert(
            "AUTHORIZATION",
            reqwest::header::HeaderValue::from_str(&std::env::var("BLOCKNATIVE_API_KEY").unwrap())
                .unwrap(), //or replace with api_key
        );
        let gas_price_estimator = BlockNative::new(
            shared::gas_price_estimation::Client(reqwest::Client::new()),
            header,
        )
        .await
        .unwrap();
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &gas_price_estimator,
            max_additional_tip: Some(3.0),
            gas_price_cap: 100e9,
            additional_tip_percentage_of_max_fee: Some(0.05),
            pending_gas_price: None,
        };
        let access_list_estimator = Arc::new(
            create_priority_estimator(
                &Client::new(),
                &web3,
                &[AccessListEstimatorType::Web3],
                None,
                None,
                "1".to_string(),
            )
            .await
            .unwrap(),
        );

        let settlement = Settlement::new(Default::default());
        let gas_estimate =
            crate::settlement_simulation::simulate_and_estimate_gas_at_current_block(
                std::iter::once((account.clone(), settlement.clone(), None)),
                &contract,
                &web3,
                Default::default(),
            )
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .unwrap();

        let submitted_transactions = Default::default();

        let submitter = Submitter::new(
            &contract,
            &account,
            &flashbots_api,
            &gas_price_estimator,
            access_list_estimator.as_ref(),
            submitted_transactions,
        )
        .unwrap();

        let params = SubmitterParams {
            target_confirm_time: Duration::from_secs(0),
            gas_estimate,
            deadline: Some(Instant::now() + Duration::from_secs(90)),
            retry_interval: Duration::from_secs(5),
            network_id: "1".to_string(),
        };
        let result = submitter.submit(settlement, params).await;
        tracing::debug!("finished with result {:?}", result);
    }

    #[test]
    fn gas_price_estimator_no_tip_test() {
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &FakeGasPriceEstimator::default(),
            additional_tip_percentage_of_max_fee: Some(5.),
            max_additional_tip: Some(10.),
            gas_price_cap: 0.,
            pending_gas_price: None,
        };

        let gas_price_estimator = gas_price_estimator.with_additional_tip(None);
        assert_eq!(gas_price_estimator.max_additional_tip, None);
    }
}
