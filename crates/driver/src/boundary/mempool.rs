use {
    crate::{
        boundary::{self, Result},
        domain::{competition::solution::settlement, eth},
        infra::blockchain::Ethereum,
    },
    async_trait::async_trait,
    ethcontract::{transaction::TransactionBuilder, transport::DynTransport},
    shared::http_client::HttpClientFactory,
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
pub use {gas_estimation::GasPriceEstimating, solver::settlement_submission::GlobalTxPool};

#[derive(Debug, Clone)]
pub struct Config {
    pub additional_tip_percentage: f64,
    pub max_additional_tip: Option<f64>,
    pub gas_price_cap: f64,
    pub target_confirm_time: std::time::Duration,
    pub max_confirm_time: std::time::Duration,
    pub retry_interval: std::time::Duration,
    pub account: ethcontract::Account,
    pub eth: Ethereum,
    pub pool: GlobalTxPool,
}

#[derive(Debug, Clone, Copy)]
pub enum HighRisk {
    Enabled,
    Disabled,
}

// TODO Perhaps a better name for this in the future might be Relay
/// The mempool to use for publishing settlements onchain.
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
    /// The public mempool of an [`Ethereum`] node.
    pub async fn public(
        config: Config,
        high_risk: HighRisk,
        gas_price_estimator: Arc<dyn GasPriceEstimating>,
    ) -> Result<Self> {
        Ok(Self {
            submit_api: Arc::new(PublicMempoolApi::new(
                vec![boundary::web3(&config.eth)],
                matches!(high_risk, HighRisk::Disabled),
            )),
            submitted_transactions: config.pool.add_sub_pool(Strategy::PublicMempool),
            gas_price_estimator,
            config,
        })
    }

    /// The [flashbots] private mempool.
    ///
    /// [flashbots]: https://docs.flashbots.net/flashbots-auction/overview
    pub async fn flashbots(
        config: Config,
        url: reqwest::Url,
        gas_price_estimator: Arc<dyn GasPriceEstimating>,
    ) -> Result<Self> {
        Ok(Self {
            submit_api: Arc::new(FlashbotsApi::new(reqwest::Client::new(), url)?),
            submitted_transactions: config.pool.add_sub_pool(Strategy::Flashbots),
            gas_price_estimator,
            config,
        })
    }

    pub async fn send(&self, settlement: settlement::Simulated) -> Result<()> {
        let web3 = boundary::web3(&self.config.eth);
        let nonce = web3
            .eth()
            .transaction_count(self.config.account.address(), None)
            .await?;
        let gas_price_estimator = SubmitterGasPriceEstimator {
            inner: self.gas_price_estimator.as_ref(),
            gas_price_cap: self.config.gas_price_cap,
            additional_tip_percentage_of_max_fee: Some(self.config.additional_tip_percentage),
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

pub async fn gas_price_estimator(config: &Config) -> Result<Arc<dyn GasPriceEstimating>> {
    Ok(Arc::new(
        shared::gas_price_estimation::create_priority_estimator(
            &HttpClientFactory::new(&shared::http_client::Arguments {
                http_timeout: std::time::Duration::from_secs(10),
            }),
            &boundary::web3(&config.eth),
            &[shared::gas_price_estimation::GasEstimatorType::Native],
            None,
        )
        .await?,
    ))
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
