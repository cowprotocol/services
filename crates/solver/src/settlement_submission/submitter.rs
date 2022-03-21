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

use super::{SubmissionError, ESTIMATE_GAS_LIMIT_FACTOR};
use crate::{
    settlement::Settlement, settlement_access_list::AccessListEstimating,
    settlement_simulation::settle_method_builder,
};
use anyhow::{anyhow, ensure, Context, Result};
use contracts::GPv2Settlement;
use ethcontract::{
    contract::MethodBuilder, dyns::DynTransport, transaction::TransactionBuilder, Account, H160,
};
use futures::FutureExt;
use gas_estimation::{EstimatedGasPrice, GasPriceEstimating};
use primitive_types::{H256, U256};
use shared::Web3;
use std::time::{Duration, Instant};
use web3::types::{AccessList, TransactionReceipt, U64};

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
    /// Network id (mainnet, rinkeby, gnosis chain)
    pub network_id: String,
}

#[derive(Debug)]
/// Enum used to handle all kind of messages received from implementers of trait TransactionSubmitting
pub enum SubmitApiError {
    InvalidNonce,
    ReplacementTransactionUnderpriced,
    OpenEthereumTooCheapToReplace, // todo ds safe to remove after dropping OE support
    /// EDEN network will reject transactions where the maximum gas cost in ETH
    /// is over 1.0.
    EdenTransactionTooExpensive,
    Other(anyhow::Error),
}

impl From<anyhow::Error> for SubmitApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}

#[derive(Debug)]
pub enum SubmissionLoopStatus {
    Enabled(AdditionalTip),
    Disabled(DisabledReason),
}

#[derive(Debug)]
pub enum AdditionalTip {
    Off,
    On,
}

#[derive(Debug)]
pub enum DisabledReason {
    MevExtractable,
}

#[derive(Debug, Clone, Copy)]
pub struct TransactionHandle {
    pub handle: H256,
    pub tx_hash: H256,
}

#[derive(Debug, Clone)]
pub struct CancelHandle {
    /// transaction previosly submitted using TransactionSubmitting::submit_transaction()
    pub submitted_transaction: TransactionHandle,
    /// empty transaction with the same nonce used for cancelling the previously submitted transaction
    pub noop_transaction: TransactionBuilder<DynTransport>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait TransactionSubmitting: Send + Sync {
    /// Submits transation to the specific network (public mempool, eden, flashbots...).
    /// Returns transaction handle
    async fn submit_transaction(
        &self,
        tx: TransactionBuilder<DynTransport>,
    ) -> Result<TransactionHandle, SubmitApiError>;
    /// Cancels already submitted transaction using the cancel handle
    async fn cancel_transaction(
        &self,
        id: &CancelHandle,
    ) -> Result<TransactionHandle, SubmitApiError>;
    /// Try to find submitted transaction from previous submission loop (in this case we don't have a TransactionHandle)
    async fn recover_pending_transaction(
        &self,
        web3: &Web3,
        address: &H160,
        nonce: U256,
    ) -> Result<Option<EstimatedGasPrice>>;
    /// Checks if transaction submitting is enabled at the moment
    fn submission_status(&self, settlement: &Settlement, network_id: &str) -> SubmissionLoopStatus;
    /// Returns displayable name of the submitter. Used for logging and metrics collection.
    fn name(&self) -> &'static str;
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
}

impl SubmitterGasPriceEstimator<'_> {
    pub fn with_no_additional_tip(&self) -> Self {
        Self {
            max_additional_tip: None,
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
    ) -> Result<EstimatedGasPrice> {
        match self.inner.estimate_with_limits(gas_limit, time_limit).await {
            Ok(mut gas_price) if gas_price.cap() <= self.gas_price_cap => {
                // boost miner tip to increase our chances of being included in a block
                if let Some(ref mut eip1559) = gas_price.eip1559 {
                    eip1559.max_priority_fee_per_gas +=
                        self.max_additional_tip.unwrap_or_default().min(
                            eip1559.max_fee_per_gas
                                * self
                                    .additional_tip_percentage_of_max_fee
                                    .unwrap_or_default(),
                        );
                }
                Ok(gas_price)
            }
            Ok(gas_price) => Err(anyhow!(
                "gas station gas price {} is larger than cap {}",
                gas_price.cap(),
                self.gas_price_cap
            )),
            Err(err) => Err(err),
        }
    }
}

pub struct Submitter<'a> {
    contract: &'a GPv2Settlement,
    account: &'a Account,
    submit_api: &'a dyn TransactionSubmitting,
    gas_price_estimator: &'a SubmitterGasPriceEstimator<'a>,
    access_list_estimator: &'a dyn AccessListEstimating,
}

impl<'a> Submitter<'a> {
    pub fn new(
        contract: &'a GPv2Settlement,
        account: &'a Account,
        submit_api: &'a dyn TransactionSubmitting,
        gas_price_estimator: &'a SubmitterGasPriceEstimator<'a>,
        access_list_estimator: &'a dyn AccessListEstimating,
    ) -> Result<Self> {
        Ok(Self {
            contract,
            account,
            submit_api,
            gas_price_estimator,
            access_list_estimator,
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

        tracing::info!(
            "starting solution submission at nonce {} with submitter {}",
            nonce,
            name
        );

        // Continually simulate and submit transactions
        let mut transactions = Vec::new();
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
                tracing::info!("stopping submission for {} because simulation failed: {:?}", name, method_error);
                Err(method_error)
            },
            new_nonce = nonce_future.fuse() => {
                tracing::info!("stopping submission for {} because account nonce changed to {}", name, new_nonce);
                Ok(None)
            },
            _ = deadline_future.fuse() => {
                tracing::info!("stopping submission for {} because deadline has been reached. cancelling last submitted transaction...", name);

                if let Some((transaction, gas_price)) = transactions.last() {
                    let gas_price = gas_price.bump(1.125).ceil();
                    match self
                        .cancel_transaction(transaction, &gas_price, nonce)
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
            const MINED_TX_PROPAGATE_TIME: Duration = Duration::from_secs(20);
            const MINED_TX_CHECK_INTERVAL: Duration = Duration::from_secs(5);
            let tx_to_propagate_deadline = Instant::now() + MINED_TX_PROPAGATE_TIME;

            tracing::info!(
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
                    tracing::info!("{} found mined transaction {:?}", name, receipt);
                    return status(receipt);
                }
                if Instant::now() + MINED_TX_CHECK_INTERVAL > tx_to_propagate_deadline {
                    break;
                }
                tokio::time::sleep(MINED_TX_CHECK_INTERVAL).await;
            }
        }

        tracing::info!("{} did not find any mined transaction", name);
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
        transactions: &mut Vec<(TransactionHandle, EstimatedGasPrice)>,
    ) -> SubmissionError {
        let submitter_name = self.submit_api.name();
        let target_confirm_time = Instant::now() + params.target_confirm_time;

        // Try to find submitted transaction from previous submission loop (with the same address and nonce)
        let pending_gas_price = self
            .submit_api
            .recover_pending_transaction(
                &self.contract.raw_instance().web3(),
                &self.account.address(),
                nonce,
            )
            .await
            .unwrap_or(None);

        loop {
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
                SubmissionLoopStatus::Enabled(AdditionalTip::Off) => {
                    self.gas_price_estimator.with_no_additional_tip()
                }
                SubmissionLoopStatus::Enabled(AdditionalTip::On) => {
                    self.gas_price_estimator.clone()
                }
            };
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

            // simulate transaction

            if let Err(err) = method.clone().view().call().await {
                if let Some((previous_tx, _)) = transactions.last() {
                    match self
                        .cancel_transaction(previous_tx, &gas_price, nonce)
                        .await
                    {
                        Ok(handle) => transactions.push((handle, gas_price)),
                        Err(err) => tracing::warn!("cancellation failed: {:?}", err),
                    }
                }
                return SubmissionError::from(err);
            }

            // if gas price has not increased enough, skip submitting the transaction.

            // Doing `.or(pending_gas_price.as_ref())` is a warning on Rust 1.58 but doing or_else
            // is a warning on 1.59. So silence the warning on the older compiler until everyone has
            // upgraded.
            #[allow(clippy::or_fun_call)]
            if let Some(previous_gas_price) = transactions
                .last()
                .map(|(_, previous_gas_price)| previous_gas_price)
                .or(pending_gas_price.as_ref())
            {
                let previous_gas_price = previous_gas_price.bump(1.125).ceil();
                if gas_price.tip() < previous_gas_price.tip()
                    || gas_price.cap() < previous_gas_price.cap()
                {
                    tokio::time::sleep(params.retry_interval).await;
                    continue;
                }
            }

            tracing::info!(
                "creating transaction with gas price (base_fee={}, max_fee={}, tip={}), gas estimate {}, submitter name: {}",
                gas_price.base_fee(),
                gas_price.cap(),
                gas_price.tip(),
                params.gas_estimate,
                submitter_name,
            );

            // execute transaction

            match self.submit_api.submit_transaction(method.tx).await {
                Ok(handle) => {
                    tracing::info!(
                        submitter = %submitter_name, ?handle,
                        "submitted transaction",
                    );
                    transactions.push((handle, gas_price));
                }
                Err(err) => match err {
                    SubmitApiError::InvalidNonce => {
                        tracing::warn!("{} submission failed: invalid nonce", submitter_name)
                    }
                    SubmitApiError::ReplacementTransactionUnderpriced => {
                        tracing::warn!(
                            "{} submission failed: replacement transaction underpriced",
                            submitter_name
                        )
                    }
                    SubmitApiError::OpenEthereumTooCheapToReplace => {
                        tracing::debug!("{} submission failed: OE has different replacement rules than our algorithm", submitter_name)
                    }
                    SubmitApiError::EdenTransactionTooExpensive => {
                        tracing::warn!(
                            "{} submission failed: eden transaction too expensive",
                            submitter_name
                        )
                    }
                    SubmitApiError::Other(err) => {
                        tracing::error!("{} submission failed: {:?}", submitter_name, err)
                    }
                },
            }
            tokio::time::sleep(params.retry_interval).await;
        }
    }

    /// Prepare transaction for simulation
    async fn build_method(
        &self,
        settlement: Settlement,
        gas_price: &EstimatedGasPrice,
        nonce: U256,
        gas_limit: f64,
    ) -> MethodBuilder<DynTransport, ()> {
        let method = settle_method_builder(self.contract, settlement.into(), self.account.clone())
            .nonce(nonce)
            .gas(U256::from_f64_lossy(gas_limit))
            .gas_price(crate::into_gas_price(gas_price));
        match self.estimate_access_list(&method.tx).await {
            Ok(access_list) => method.access_list(access_list),
            Err(err) => {
                tracing::info!("access list not used, reason: {:?}", err);
                method
            }
        }
    }

    /// Estimate access list and validate
    async fn estimate_access_list(
        &self,
        tx: &TransactionBuilder<DynTransport>,
    ) -> Result<AccessList> {
        let access_list = self.access_list_estimator.estimate_access_list(tx).await?;
        let (gas_before_access_list, gas_after_access_list) = futures::try_join!(
            tx.clone().estimate_gas(),
            tx.clone().access_list(access_list.clone()).estimate_gas()
        )?;

        tracing::debug!(
            "gas before/after access list: {}/{}, access_list: {:?}",
            gas_before_access_list,
            gas_after_access_list,
            access_list,
        );
        ensure!(
            gas_before_access_list > gas_after_access_list,
            "access list exist but does not lower the gas usage"
        );
        Ok(access_list)
    }

    /// Prepare noop transaction. This transaction does transfer of 0 value to self and always spends 21000 gas.
    fn build_noop_transaction(
        &self,
        gas_price: &EstimatedGasPrice,
        nonce: U256,
    ) -> TransactionBuilder<DynTransport> {
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
        transaction: &TransactionHandle,
        gas_price: &EstimatedGasPrice,
        nonce: U256,
    ) -> Result<TransactionHandle, SubmitApiError> {
        let cancel_handle = CancelHandle {
            submitted_transaction: *transaction,
            noop_transaction: self.build_noop_transaction(&gas_price.bump(3.), nonce),
        };
        self.submit_api.cancel_transaction(&cancel_handle).await
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
        };
        let access_list_estimator = Arc::new(
            create_priority_estimator(
                &Client::new(),
                &web3,
                &[AccessListEstimatorType::Web3],
                None,
                None,
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

        let submitter = Submitter::new(
            &contract,
            &account,
            &flashbots_api,
            &gas_price_estimator,
            access_list_estimator.as_ref(),
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
        tracing::info!("finished with result {:?}", result);
    }

    #[test]
    fn gas_price_estimator_no_tip_test() {
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: &FakeGasPriceEstimator::default(),
            additional_tip_percentage_of_max_fee: Some(5.),
            max_additional_tip: Some(10.),
            gas_price_cap: 0.,
        };

        let gas_price_estimator = gas_price_estimator.with_no_additional_tip();
        assert_eq!(gas_price_estimator.max_additional_tip, None);
    }
}
