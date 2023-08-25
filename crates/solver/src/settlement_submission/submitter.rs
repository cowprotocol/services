// Design:
// As in the traditional transaction submission workflow the main work in this
// module is checking the gas price in a loop and updating the transaction when
// the gas price increases. This differs so that we can make use of the property
// that flashbots transactions do not cost gas if they fail. When we detect that
// the transaction would no longer succeed we stop trying to submit and return
// so that the solver can run again.
// In addition to simulation failure we make use of a deadline after which
// submission attempts also stop. This allows the solver to update and improve a
// solution even if it hasn't yet become invalid.
// We do not know in advance which of our submitted transactions will get mined.
// Instead of polling all of them we only check the account's nonce as an
// optimization. When this happens all our transactions definitely become
// invalid (even if the transaction came for whatever reason from outside) so it
// is only at that point that we need to check the hashes individually to the
// find the one that got mined (if any).

mod common;
pub mod eden_api;
pub mod flashbots_api;
pub mod public_mempool_api;

use {
    super::{SubTxPoolRef, SubmissionError},
    crate::{
        settlement::Settlement,
        settlement_access_list::{estimate_settlement_access_list, AccessListEstimating},
        settlement_simulation::settle_method_builder,
        settlement_submission::gas_limit_for_estimate,
    },
    anyhow::{anyhow, ensure, Context, Result},
    contracts::GPv2Settlement,
    ethcontract::{contract::MethodBuilder, transaction::TransactionBuilder, Account},
    futures::FutureExt,
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    primitive_types::{H256, U256},
    shared::{
        code_fetching::CodeFetching,
        conversions::into_gas_price,
        ethrpc::{Web3, Web3Transport},
        http_solver::model::InternalizationStrategy,
        submitter_constants::{TX_ALREADY_KNOWN, TX_ALREADY_MINED},
    },
    std::{
        fmt,
        time::{Duration, Instant},
    },
    strum::IntoStaticStr,
    web3::types::{AccessList, TransactionReceipt, U64},
};

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
    /// Re-simulate and resend transaction on every retry_interval seconds
    pub retry_interval: Duration,
    /// Network id (mainnet, rinkeby, goerli, gnosis chain)
    pub network_id: String,
    /// Additional bytes to append to the call data. This is required by the
    /// `driver`.
    pub additional_call_data: Vec<u8>,
    pub use_soft_cancellations: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SubmissionLoopStatus {
    Enabled,
    Disabled(DisabledReason),
}

#[derive(Debug, Clone, Copy, IntoStaticStr)]
pub enum Strategy {
    Eden,
    Flashbots,
    PublicMempool,
}

impl fmt::Display for Strategy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
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
    /// Submits transaction to the specific network (public mempool, eden,
    /// flashbots...). Returns transaction handle
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
    /// Additionally increase max_priority_fee_per_gas by percentage of
    /// max_fee_per_gas, in order to increase the chances of a transaction being
    /// mined
    pub additional_tip_percentage_of_max_fee: f64,
    /// Maximum max_priority_fee_per_gas additional increase
    pub max_additional_tip: f64,
    /// Maximum max_fee_per_gas to pay for a transaction
    pub max_fee_per_gas: f64,
}

impl SubmitterGasPriceEstimator<'_> {
    pub fn with_additional_tip(&self, max_additional_tip: f64) -> Self {
        Self {
            max_additional_tip,
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
        let mut estimate = self
            .inner
            .estimate_with_limits(gas_limit, time_limit)
            .await?;

        estimate.max_fee_per_gas = estimate.max_fee_per_gas.min(self.max_fee_per_gas);
        estimate.max_priority_fee_per_gas += self
            .max_additional_tip
            .min(estimate.max_fee_per_gas * self.additional_tip_percentage_of_max_fee);
        estimate.max_priority_fee_per_gas = estimate
            .max_priority_fee_per_gas
            .min(estimate.max_fee_per_gas);
        estimate = estimate.ceil();

        ensure!(estimate.is_valid(), "gas estimate exceeds cap {estimate:?}");
        Ok(estimate)
    }
}

pub struct Submitter<'a> {
    contract: &'a GPv2Settlement,
    account: &'a Account,
    nonce: U256,
    submit_api: &'a dyn TransactionSubmitting,
    gas_price_estimator: &'a SubmitterGasPriceEstimator<'a>,
    access_list_estimator: &'a dyn AccessListEstimating,
    code_fetcher: &'a dyn CodeFetching,
    submitted_transactions: SubTxPoolRef,
    web3: Web3,
}

impl<'a> Submitter<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract: &'a GPv2Settlement,
        account: &'a Account,
        nonce: U256,
        submit_api: &'a dyn TransactionSubmitting,
        gas_price_estimator: &'a SubmitterGasPriceEstimator<'a>,
        access_list_estimator: &'a dyn AccessListEstimating,
        submitted_transactions: SubTxPoolRef,
        web3: Web3,
        code_fetcher: &'a dyn CodeFetching,
    ) -> Result<Self> {
        Ok(Self {
            contract,
            account,
            nonce,
            submit_api,
            gas_price_estimator,
            access_list_estimator,
            submitted_transactions,
            web3,
            code_fetcher,
        })
    }
}

impl<'a> Submitter<'a> {
    /// Submit a settlement to the contract, updating the transaction with gas
    /// prices if they increase.
    ///
    /// Only works on mainnet.
    pub async fn submit(
        &self,
        settlement: Settlement,
        params: SubmitterParams,
    ) -> Result<TransactionReceipt, SubmissionError> {
        let name = self.submit_api.name();
        let use_soft_cancellations = params.use_soft_cancellations;

        tracing::debug!(address=?self.account.address(), ?self.nonce, "starting solution submission");

        self.submitted_transactions.remove_older_than(self.nonce);

        // Take pending transactions from previous submission loops with the same nonce
        // Those exist if
        // 1. Previous loop timed out and no transaction was mined
        // 2. Previous loop ended with simulation revert, and cancellation tx was sent
        // but not mined
        let mut transactions = self
            .submitted_transactions
            .get(self.account.address(), self.nonce)
            .unwrap_or_default();

        let deadline = params.deadline;

        // Continually simulate and submit transactions
        let submit_future = self.submit_with_increasing_gas_prices_until_simulation_fails(
            settlement,
            params,
            &mut transactions,
        );

        // Nonce future is used to detect if tx is mined
        let nonce_future = self.wait_for_nonce_to_change(self.nonce);

        // If specified, deadline future stops submitting when deadline is reached
        let deadline_future = tokio::time::sleep(match deadline {
            Some(deadline) => deadline.saturating_duration_since(Instant::now()),
            None => Duration::from_secs(u64::MAX),
        });

        let fallback_result = tokio::select! {
            method_error = submit_future => {
                tracing::debug!("stopping submission because simulation failed: {:?}", method_error);
                Err(method_error)
            },
            new_nonce = nonce_future => {
                tracing::debug!("stopping submission because account nonce changed to {}", new_nonce);
                Ok(None)
            },
            _ = deadline_future => {
                tracing::debug!("stopping submission because deadline has been reached. cancelling last submitted transaction...");
                if let Some((_, gas_price)) = transactions.last() {
                    let gas_price = gas_price.bump(GAS_PRICE_BUMP).ceil();
                    match self
                        .cancel_transaction(&gas_price, self.nonce)
                        .await
                    {
                        Ok(handle) => transactions.push((handle, gas_price)),
                        Err(err) => tracing::warn!("cancellation failed: {:?}", err),
                    }
                }
                Ok(None)
            },
        };

        // Update (overwrite) the submitted transaction list with `transactions`
        // variable that, at this point, contains both transactions from
        // previous submission loop and transactions from current submission
        // loop.
        // This is not needed when soft cancellations are used as any pending tx will
        // have been removed from the mempool and doesn't require further tracking.
        if !use_soft_cancellations {
            tracing::debug!("update list of pending tx hashes with {:?}", transactions);
            self.submitted_transactions.update(
                self.account.address(),
                self.nonce,
                transactions.clone(),
            );
        }

        // After stopping submission of new transactions we wait for some time to give a
        // potentially mined previously submitted transaction time to propagate
        // to our node.

        // Example:
        // 1. We submit tx to ethereum node, and we start counting 10s pause before new
        // loop iteration. 2. In the meantime, block A gets mined somewhere in
        // the network (not containing our tx) 3. After some time block B is
        // mined somewhere in the network (containing our tx) 4. Our node
        // receives block A. 5. Our 10s is up but our node received only block A
        // because of the delay in block propagation. We simulate tx and it fails, we
        // return back 6. If we don't wait another 20s to receive block B, we
        // wont see mined tx.
        if !transactions.is_empty() {
            const MINED_TX_PROPAGATE_TIME: Duration = Duration::from_secs(20);
            const MINED_TX_CHECK_INTERVAL: Duration = Duration::from_secs(5);
            let tx_to_propagate_deadline = Instant::now() + MINED_TX_PROPAGATE_TIME;

            tracing::debug!(
                "waiting up to {} seconds to see if a transaction was mined",
                MINED_TX_PROPAGATE_TIME.as_secs(),
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
                    tracing::debug!("found mined transaction {:?}", receipt.transaction_hash);
                    track_mined_transactions(&format!("{name}"));
                    // No need to keep submitted transactions for next auction if tx was found.
                    // This also protects against reorgs where mined tx is found in one auction but
                    // the block gets reorged and the tx is again found in the
                    // next auction.
                    self.submitted_transactions.clear();
                    return status(receipt);
                }
                if Instant::now() + MINED_TX_CHECK_INTERVAL > tx_to_propagate_deadline {
                    break;
                }
                tokio::time::sleep(MINED_TX_CHECK_INTERVAL).await;
            }
        }

        tracing::debug!("did not find any mined transaction");
        fallback_result
            .transpose()
            .unwrap_or(Err(SubmissionError::Timeout))
            .map_err(|err| {
                track_strategy_outcome(&format!("{name}"), err.as_outcome().label());
                err
            })
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

    /// Keep polling the account's nonce until it is different from
    /// initial_nonce returning the new nonce.
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

    /// Keep submitting the settlement transaction to the network as gas price
    /// changes.
    ///
    /// Returns when simulation of the transaction fails. This likely happens if
    /// the settlement becomes invalid due to changing prices or the
    /// account's nonce changes.
    ///
    /// Potential transaction hashes are communicated back through a shared
    /// vector.
    async fn submit_with_increasing_gas_prices_until_simulation_fails(
        &self,
        settlement: Settlement,
        params: SubmitterParams,
        transactions: &mut Vec<(TransactionHandle, GasPrice1559)>,
    ) -> SubmissionError {
        let target_confirm_time = Instant::now() + params.target_confirm_time;

        let mut access_list: Option<AccessList> = None;

        // Try to find submitted transaction from previous submission attempt (with the
        // same address and nonce)
        let mut pending_gas_price = transactions.last().cloned().map(|(_, gas_price)| gas_price);

        loop {
            let submission_status = self
                .submit_api
                .submission_status(&settlement, &params.network_id);
            let estimator = match submission_status {
                SubmissionLoopStatus::Disabled(reason) => {
                    tracing::debug!("strategy temporarily disabled, reason: {:?}", reason);
                    return SubmissionError::from(anyhow!("strategy temporarily disabled"));
                }
                SubmissionLoopStatus::Enabled => self.gas_price_estimator.clone(),
            };
            let gas_limit = gas_limit_for_estimate(params.gas_estimate).to_f64_lossy();
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

            let mut method =
                self.build_method(settlement.clone(), &gas_price, self.nonce, gas_limit);

            // append additional call data

            let mut data = method.tx.data.take().unwrap();
            data.0.extend(params.additional_call_data.clone());
            method.tx = method.tx.data(data);

            // append access list

            let method = match access_list.as_ref() {
                Some(access_list) => method.access_list(access_list.clone()),
                None => match self.estimate_access_list(&settlement, &method.tx).await {
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
                if let Some(previous_gas_price) = pending_gas_price {
                    // We only match the replacement gas price because we don't care about the
                    // cancellation actually being mined as long as it successfully replaces the
                    // original transaction in the mempool.
                    let replacement_price = previous_gas_price.bump(GAS_PRICE_BUMP).ceil();
                    match self
                        .cancel_transaction(&replacement_price, self.nonce)
                        .await
                    {
                        Ok(handle) => transactions.push((handle, gas_price)),
                        Err(err) => tracing::warn!("cancellation failed: {:?}", err),
                    }
                }
                return SubmissionError::from(err);
            }

            // if gas price has not increased enough, skip submitting the transaction.
            if let Some(previous_gas_price) = pending_gas_price {
                let replacement_price = previous_gas_price.bump(GAS_PRICE_BUMP).ceil();
                if gas_price.max_priority_fee_per_gas < replacement_price.max_priority_fee_per_gas
                    || gas_price.max_fee_per_gas < replacement_price.max_fee_per_gas
                {
                    tracing::debug!(
                        %gas_price,
                        %replacement_price,
                        sleep = ?params.retry_interval,
                        "keep waiting for gas price to increase enough"
                    );
                    tokio::time::sleep(params.retry_interval).await;
                    continue;
                }
            }

            tracing::debug!(%gas_price, gas_estimate=%params.gas_estimate,
                "creating transaction with gas price"
            );

            // execute transaction

            let label: &'static str = self.submit_api.name().into();
            match self.submit_api.submit_transaction(method.tx).await {
                Ok(handle) => {
                    tracing::debug!(?handle, "submitted transaction",);
                    transactions.push((handle, gas_price));
                    pending_gas_price = Some(gas_price);
                    track_submission_success(label, true);
                }
                Err(err) => {
                    let err = err.to_string();
                    if TX_ALREADY_MINED.iter().any(|msg| err.contains(msg)) {
                        // Due to a race condition we sometimes notice too late that a tx was
                        // already mined and submit once too often.
                        tracing::debug!(?err, "transaction already mined");
                        track_submission_success(label, true);
                    } else if TX_ALREADY_KNOWN.iter().any(|msg| err.contains(msg)) {
                        // This case means that the node is already aware of the tx although we
                        // didn't get any confirmation in the form of a tx handle. If that happens
                        // we simply set the current gas price as the pending gas price which means
                        // we will only try submitting again when the gas price increased by
                        // GAS_PRICE_BUMP again thus avoiding repeated "tx underpriced" errors.
                        pending_gas_price = Some(gas_price);
                        tracing::debug!(?err, "transaction already known");
                        track_submission_success(label, true);
                    } else {
                        tracing::warn!(?err, "submission failed");
                        track_submission_success(label, false);
                    }
                }
            }
            tokio::time::sleep(params.retry_interval).await;
        }
    }

    /// Prepare transaction for simulation
    fn build_method(
        &self,
        settlement: Settlement,
        gas_price: &GasPrice1559,
        nonce: U256,
        gas_limit: f64,
    ) -> MethodBuilder<Web3Transport, ()> {
        let settlement = settlement.encode(InternalizationStrategy::SkipInternalizableInteraction);
        settle_method_builder(self.contract, settlement, self.account.clone())
            .nonce(nonce)
            .gas(U256::from_f64_lossy(gas_limit))
            .gas_price(into_gas_price(gas_price))
    }

    /// Estimate access list and validate
    async fn estimate_access_list(
        &self,
        settlement: &Settlement,
        tx: &TransactionBuilder<Web3Transport>,
    ) -> Result<AccessList> {
        let access_list = estimate_settlement_access_list(
            self.access_list_estimator,
            self.code_fetcher,
            self.web3.clone(),
            self.account.clone(),
            settlement,
            tx,
        )
        .await?;
        let (without_access_list, with_access_list) = futures::join!(
            // This call will fail for orders paying ETH to SC wallets
            tx.clone().estimate_gas(),
            tx.clone().access_list(access_list.clone()).estimate_gas()
        );

        match (without_access_list, with_access_list) {
            (Err(_), Ok(_)) => {
                tracing::debug!("using an access list made the transaction executable");
                Ok(access_list)
            }
            (Ok(_), Err(_)) => {
                anyhow::bail!("access list caused the transaction to fail");
            }
            (Ok(gas_without), Ok(gas_with)) => {
                ensure!(
                    gas_without > gas_with,
                    "access list exists but does not lower the gas usage"
                );
                let gas_percent_saved = (gas_without.to_f64_lossy() - gas_with.to_f64_lossy())
                    / gas_without.to_f64_lossy()
                    * 100.;
                tracing::debug!(
                    "gas before/after access list: {}/{}, access_list: {:?}, gas percent saved: {}",
                    gas_without,
                    gas_with,
                    access_list,
                    gas_percent_saved
                );
                Ok(access_list)
            }
            (Err(_), Err(_)) => {
                anyhow::bail!("transaction would revert with and without access list");
            }
        }
    }

    /// Prepare noop transaction. This transaction does transfer of 0 value to
    /// self and always spends 21000 gas.
    fn build_noop_transaction(
        &self,
        gas_price: &GasPrice1559,
        nonce: U256,
    ) -> TransactionBuilder<Web3Transport> {
        TransactionBuilder::new(self.contract.raw_instance().web3())
            .from(self.account.clone())
            .to(self.account.address())
            .nonce(nonce)
            .gas_price(into_gas_price(gas_price))
            .gas(21000.into())
    }

    /// Prepare all data needed for cancellation of previously submitted
    /// transaction and execute cancellation.
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
    // successful transaction
    Ok(receipt)
}

/// From a list of potential hashes find one that was mined.
async fn find_mined_transaction(web3: &Web3, hashes: &[H256]) -> Option<TransactionReceipt> {
    // It would be nice to use the nonce and account address to find the transaction
    // hash but there is no way to do this in ethrpc api so we have to check the
    // candidates one by one.
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
    /// Tracks how many transactions get successfully submitted with the
    /// different submission strategies.
    #[metric(labels("submitter", "result"))]
    submissions: prometheus::IntCounterVec,
    /// Tracks how many transactions get successfully mined by the different
    /// submission strategies.
    #[metric(labels("submitter"))]
    mined_transactions: prometheus::IntCounterVec,
    /// Settlement submission outcomes for each strategy
    #[metric(labels("strategy", "result"))]
    strategy_outcomes: prometheus::IntCounterVec,
}

pub(crate) fn track_submission_success(submitter: &str, was_successful: bool) {
    let result = if was_successful { "success" } else { "error" };
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
        .submissions
        .with_label_values(&[submitter, result])
        .inc();
}

fn track_mined_transactions(submitter: &str) {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
        .mined_transactions
        .with_label_values(&[submitter])
        .inc();
}

fn track_strategy_outcome(strategy: &str, outcome: &str) {
    Metrics::instance(observe::metrics::get_storage_registry())
        .expect("unexpected error getting metrics instance")
        .strategy_outcomes
        .with_label_values(&[strategy, outcome])
        .inc();
}
#[cfg(test)]
mod tests {
    use {
        super::{super::submitter::flashbots_api::FlashbotsApi, *},
        crate::settlement_access_list::{create_priority_estimator, AccessListEstimatorType},
        ethcontract::PrivateKey,
        gas_estimation::blocknative::BlockNative,
        reqwest::Client,
        shared::{
            code_fetching::MockCodeFetching,
            ethrpc::create_env_test_transport,
            gas_price_estimation::FakeGasPriceEstimator,
        },
        std::sync::Arc,
        tracing::level_filters::LevelFilter,
    };

    #[tokio::test]
    #[ignore]
    async fn flashbots_mainnet_settlement() {
        observe::tracing::initialize("solver=debug,shared=debug", LevelFilter::OFF);

        let web3 = Web3::new(create_env_test_transport());
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        assert_eq!(chain_id, 1);
        let private_key: PrivateKey = std::env::var("PRIVATE_KEY").unwrap().parse().unwrap();
        let account = Account::Offline(private_key, Some(chain_id));
        let nonce = web3
            .eth()
            .transaction_count(account.address(), None)
            .await
            .unwrap();
        let contract = GPv2Settlement::deployed(&web3).await.unwrap();
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
            max_additional_tip: 3.0,
            max_fee_per_gas: 100e9,
            additional_tip_percentage_of_max_fee: 0.05,
        };
        let access_list_estimator = Arc::new(
            create_priority_estimator(
                &web3,
                &[AccessListEstimatorType::Web3],
                None,
                "1".to_string(),
            )
            .unwrap(),
        );
        let code_fetcher = MockCodeFetching::new();

        let settlement = Settlement::new(Default::default());
        let gas_estimate =
            crate::settlement_simulation::simulate_and_estimate_gas_at_current_block(
                std::iter::once((
                    account.clone(),
                    settlement
                        .clone()
                        .encode(InternalizationStrategy::SkipInternalizableInteraction),
                    None,
                )),
                &contract,
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
            nonce,
            &flashbots_api,
            &gas_price_estimator,
            access_list_estimator.as_ref(),
            submitted_transactions,
            web3.clone(),
            &code_fetcher,
        )
        .unwrap();

        let params = SubmitterParams {
            target_confirm_time: Duration::from_secs(0),
            gas_estimate,
            deadline: Some(Instant::now() + Duration::from_secs(90)),
            retry_interval: Duration::from_secs(5),
            network_id: "1".to_string(),
            additional_call_data: Default::default(),
            use_soft_cancellations: false,
        };
        let result = submitter.submit(settlement, params).await;
        tracing::debug!("finished with result {:?}", result);
    }

    #[tokio::test]
    async fn gas_price_estimator_includes_additional_tip() {
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &FakeGasPriceEstimator::new(GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 500.,
                max_priority_fee_per_gas: 1.,
            }),
            additional_tip_percentage_of_max_fee: 0.05,
            max_additional_tip: 1000.,
            max_fee_per_gas: 200.,
        };

        assert_eq!(
            gas_price_estimator.estimate().await.unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 200.,
                max_priority_fee_per_gas: 11.,
            }
        );
    }

    #[tokio::test]
    async fn gas_price_estimator_additional_tip_gets_capped() {
        // Capped by `max_additional_tip`.
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &FakeGasPriceEstimator::new(GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 500.,
                max_priority_fee_per_gas: 1.,
            }),
            additional_tip_percentage_of_max_fee: 0.5,
            max_additional_tip: 5.,
            max_fee_per_gas: 200.,
        };

        assert_eq!(
            gas_price_estimator.estimate().await.unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 200.,
                max_priority_fee_per_gas: 6.,
            }
        );

        // Capped by `max_fee_per_gas`.
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &FakeGasPriceEstimator::new(GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 500.,
                max_priority_fee_per_gas: 1.,
            }),
            additional_tip_percentage_of_max_fee: 5.,
            max_additional_tip: 1000.,
            max_fee_per_gas: 200.,
        };

        assert_eq!(
            gas_price_estimator.estimate().await.unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 200.,
                max_priority_fee_per_gas: 200.,
            }
        );
    }

    #[test]
    fn gas_price_estimator_no_tip_test() {
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &FakeGasPriceEstimator::default(),
            additional_tip_percentage_of_max_fee: 5.,
            max_additional_tip: 10.,
            max_fee_per_gas: 0.,
        };

        assert_eq!(
            gas_price_estimator
                .with_additional_tip(0.)
                .max_additional_tip,
            0.
        );
        assert_eq!(gas_price_estimator.max_additional_tip, 10.);
    }
}
