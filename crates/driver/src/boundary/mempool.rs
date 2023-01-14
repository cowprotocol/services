pub use solver::settlement_submission::GlobalTxPool;
use {
    crate::{
        boundary::Result,
        domain::{competition::solution::settlement, eth},
        infra::blockchain::Ethereum,
    },
    async_trait::async_trait,
    ethcontract::{transaction::TransactionBuilder, transport::DynTransport},
    gas_estimation::GasPriceEstimating,
    shared::gas_price_estimation::FakeGasPriceEstimator,
    solver::{
        settlement_access_list::AccessListEstimating,
        settlement_submission::{
            submitter::{
                flashbots_api::FlashbotsApi,
                public_mempool_api::PublicMempoolApi,
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
    web3::types::AccessList,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub additional_tip_percentage_of_max_fee: Option<f64>,
    pub max_additional_tip: Option<f64>,
    pub gas_price_cap: f64,
    pub target_confirm_time: std::time::Duration,
    pub max_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    pub account: ethcontract::Account,
    pub high_risk_disabled: bool,
    pub eth: Ethereum,
    pub pool: GlobalTxPool,
}

#[derive(Clone)]
pub struct Mempool {
    config: Config,
    submit_api: Arc<dyn TransactionSubmitting>,
    gas_price_estimator: Arc<dyn GasPriceEstimating>,
    submitted_transactions: SubTxPoolRef,
}

impl std::fmt::Debug for Mempool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mempool")
            .field("config", &self.config)
            .finish()
    }
}

impl Mempool {
    pub fn public(config: Config) -> Self {
        Self {
            submit_api: Arc::new(PublicMempoolApi::new(
                vec![config.eth.web3()],
                config.high_risk_disabled,
            )),
            submitted_transactions: config.pool.add_sub_pool(Strategy::PublicMempool),
            config,
            // TODO Follow-up PR: use shared::gas_price_estimation::create_priority_estimator for
            // this
            gas_price_estimator: Arc::new(FakeGasPriceEstimator::new(Default::default())),
        }
    }

    pub fn flashbots(config: Config, url: reqwest::Url) -> Result<Self> {
        Ok(Self {
            submit_api: Arc::new(FlashbotsApi::new(reqwest::Client::new(), url)?),
            submitted_transactions: config.pool.add_sub_pool(Strategy::Flashbots),
            config,
            // TODO Follow-up PR: use shared::gas_price_estimation::create_priority_estimator for
            // this
            gas_price_estimator: Arc::new(FakeGasPriceEstimator::new(Default::default())),
        })
    }

    pub async fn send(&self, settlement: settlement::Simulated) -> Result<()> {
        let web3 = self.config.eth.web3();
        let nonce = web3
            .eth()
            .transaction_count(self.config.account.address(), None)
            .await?;
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: self.gas_price_estimator.as_ref(),
            gas_price_cap: self.config.gas_price_cap,
            additional_tip_percentage_of_max_fee: self.config.additional_tip_percentage_of_max_fee,
            max_additional_tip: self.config.max_additional_tip,
        };
        let estimator = AccessListEstimator(settlement.access_list.clone());
        let submitter = Submitter::new(
            self.config.eth.contracts().settlement(),
            &self.config.account,
            nonce,
            self.submit_api.as_ref(),
            &gas_price_estimator,
            &estimator,
            self.submitted_transactions.clone(),
            web3.clone(),
            &web3,
        )?;
        let gas = settlement.gas;
        submitter
            .submit(
                settlement.boundary().inner,
                SubmitterParams {
                    target_confirm_time: self.config.target_confirm_time,
                    gas_estimate: gas.into(),
                    deadline: Some(std::time::Instant::now() + self.config.max_confirm_time),
                    retry_interval: self.config.retry_interval,
                    network_id: self.config.eth.network_id().to_string(),
                },
            )
            .await?;
        Ok(())
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
