use {
    crate::{
        boundary::{self, Result},
        domain::{competition::solution::settlement::Settlement, eth},
        infra::{blockchain::Ethereum, solver::Solver},
    },
    async_trait::async_trait,
    ethcontract::{transaction::TransactionBuilder, transport::DynTransport},
    shared::http_client::HttpClientFactory,
    solver::{
        settlement_access_list::AccessListEstimating,
        settlement_submission::{
            submitter::{
                flashbots_api::FlashbotsApi,
                public_mempool_api::{PublicMempoolApi, SubmissionNode, SubmissionNodeKind},
                Strategy,
                Submitter,
                SubmitterGasPriceEstimator,
                SubmitterParams,
                TransactionSubmitting,
            },
            SubTxPoolRef,
        },
    },
    std::{fmt::Debug, sync::Arc},
    tracing::Instrument,
    web3::types::AccessList,
};
pub use {gas_estimation::GasPriceEstimating, solver::settlement_submission::GlobalTxPool};

#[derive(Debug, Clone)]
pub struct Config {
    pub additional_tip_percentage: f64,
    pub gas_price_cap: f64,
    pub target_confirm_time: std::time::Duration,
    pub max_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    pub kind: Kind,
}

#[derive(Debug, Clone)]
pub enum Kind {
    /// The public mempool of the [`Ethereum`] node.
    Public(RevertProtection),
    /// The MEVBlocker private mempool.
    MEVBlocker {
        url: reqwest::Url,
        max_additional_tip: f64,
        use_soft_cancellations: bool,
    },
}

impl Kind {
    /// for instrumentization purposes
    pub fn format_variant(&self) -> &'static str {
        match self {
            Kind::Public(_) => "PublicMempool",
            Kind::MEVBlocker { .. } => "MEVBlocker",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum RevertProtection {
    Enabled,
    Disabled,
}

/// The mempool to use for publishing settlements to the blockchain.
#[derive(Clone)]
pub struct Mempool {
    config: Config,
    submit_api: Arc<dyn TransactionSubmitting>,
    gas_price_estimator: Arc<dyn GasPriceEstimating>,
    submitted_transactions: SubTxPoolRef,
    eth: Ethereum,
}

impl std::fmt::Debug for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mempool")
            .field("config", &self.config)
            .finish()
    }
}

impl std::fmt::Display for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mempool({})", self.config.kind.format_variant())
    }
}

impl Mempool {
    pub async fn new(config: Config, eth: Ethereum, pool: GlobalTxPool) -> Result<Self> {
        let gas_price_estimator = Arc::new(
            shared::gas_price_estimation::create_priority_estimator(
                &HttpClientFactory::new(&shared::http_client::Arguments {
                    http_timeout: std::time::Duration::from_secs(10),
                }),
                &boundary::web3(&eth),
                &[shared::gas_price_estimation::GasEstimatorType::Native],
                None,
            )
            .await?,
        );
        Ok(match &config.kind {
            Kind::Public(revert_protection) => Self {
                submit_api: Arc::new(PublicMempoolApi::new(
                    vec![SubmissionNode::new(
                        SubmissionNodeKind::Broadcast,
                        boundary::web3(&eth),
                    )],
                    matches!(revert_protection, RevertProtection::Enabled),
                )),
                submitted_transactions: pool.add_sub_pool(Strategy::PublicMempool),
                gas_price_estimator,
                config,
                eth,
            },
            Kind::MEVBlocker { url, .. } => Self {
                submit_api: Arc::new(FlashbotsApi::new(reqwest::Client::new(), url.to_owned())?),
                submitted_transactions: pool.add_sub_pool(Strategy::Flashbots),
                gas_price_estimator,
                config,
                eth,
            },
        })
    }

    /// Publish the settlement and wait for it to be confirmed.
    pub async fn execute(&self, solver: &Solver, settlement: Settlement) -> Result<eth::TxId> {
        let web3 = boundary::web3(&self.eth);
        let nonce = web3
            .eth()
            .transaction_count(solver.address().into(), None)
            .await?;
        let max_fee_per_gas = eth::U256::from(settlement.gas.price).to_f64_lossy();
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: self.gas_price_estimator.as_ref(),
            max_fee_per_gas: max_fee_per_gas.min(self.config.gas_price_cap),
            additional_tip_percentage_of_max_fee: self.config.additional_tip_percentage,
            max_additional_tip: match self.config.kind {
                Kind::Public(_) => 0.,
                Kind::MEVBlocker {
                    max_additional_tip, ..
                } => max_additional_tip,
            },
        };
        let use_soft_cancellations = match self.config.kind {
            Kind::Public(_) => false,
            Kind::MEVBlocker {
                use_soft_cancellations,
                ..
            } => use_soft_cancellations,
        };
        let estimator = AccessListEstimator(settlement.access_list.clone());
        // TODO: move tx submission logic from legacy code into the driver (#1543)
        let account = solver.account();
        let submitter = Submitter::new(
            self.eth.contracts().settlement(),
            &account,
            nonce,
            self.submit_api.as_ref(),
            &gas_price_estimator,
            &estimator,
            self.submitted_transactions.clone(),
            web3.clone(),
            &web3,
        )?;
        let receipt = submitter
            .submit(
                settlement.boundary.inner,
                SubmitterParams {
                    target_confirm_time: self.config.target_confirm_time,
                    gas_estimate: settlement.gas.estimate.into(),
                    deadline: Some(std::time::Instant::now() + self.config.max_confirm_time),
                    retry_interval: self.config.retry_interval,
                    network_id: self.eth.network().id.to_string(),
                    additional_call_data: settlement.auction_id.to_be_bytes().into_iter().collect(),
                    use_soft_cancellations,
                },
            )
            .instrument(tracing::info_span!(
                "mempool",
                kind = self.config.kind.format_variant()
            ))
            .await?;
        Ok(receipt.transaction_hash.into())
    }

    pub fn config(&self) -> &Config {
        &self.config
    }
}

struct AccessListEstimator(eth::AccessList);

#[async_trait]
impl AccessListEstimating for AccessListEstimator {
    async fn estimate_access_lists(
        &self,
        txs: &[TransactionBuilder<DynTransport>],
        _partial_access_list: Option<AccessList>,
    ) -> Result<Vec<Result<AccessList>>> {
        let mut result = Vec::new();
        result.resize_with(txs.len(), || Ok(self.0.clone().into()));
        Ok(result)
    }
}
