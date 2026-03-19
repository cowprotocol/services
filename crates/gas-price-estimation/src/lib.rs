pub mod configurable_alloy;
pub mod driver;
pub mod eth_node;
pub mod fake;
pub mod gas_price;
pub mod priority;

use {
    crate::{
        configurable_alloy::{
            ConfigurableGasPriceEstimator,
            EstimatorConfig,
            default_past_blocks,
            default_reward_percentile,
        },
        eth_node::NodeGasPriceEstimator,
        priority::PriorityGasPriceEstimating,
    },
    alloy_eips::eip1559::{Eip1559Estimation, calc_effective_gas_price},
    alloy_provider::Provider,
    anyhow::Result,
    ethrpc::Web3,
    tracing::instrument,
    url::Url,
};
pub use {driver::DriverGasEstimator, fake::FakeGasPriceEstimator};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait GasPriceEstimating: Send + Sync {
    /// Estimate the gas price for a transaction to be mined "quickly".
    async fn estimate(&self) -> Result<Eip1559Estimation>;

    async fn base_fee(&self) -> Result<Option<u64>>;

    async fn effective_gas_price(&self) -> Result<u128> {
        let estimate = self.estimate().await?;
        let base_fee = self.base_fee().await?;
        Ok(calc_effective_gas_price(
            estimate.max_fee_per_gas,
            estimate.max_priority_fee_per_gas,
            base_fee,
        ))
    }
}

#[derive(Clone, Debug)]
pub enum GasEstimatorType {
    Web3,
    Driver(Url),
    Alloy,
}

#[instrument(skip_all)]
pub async fn create_priority_estimator(
    http_client: reqwest::Client,
    web3: &Web3,
    estimator_types: &[GasEstimatorType],
) -> Result<impl GasPriceEstimating + use<>> {
    let network_id = web3.provider.get_chain_id().await?.to_string();
    let mut estimators = Vec::<Box<dyn GasPriceEstimating>>::new();

    for estimator_type in estimator_types {
        tracing::info!("estimator {estimator_type:?}, networkid {network_id}");
        match estimator_type {
            GasEstimatorType::Driver(url) => {
                estimators.push(Box::new(DriverGasEstimator::new(
                    http_client.clone(),
                    url.clone(),
                    web3.provider.clone(),
                )));
            }
            GasEstimatorType::Web3 => {
                estimators.push(Box::new(NodeGasPriceEstimator::new(web3.provider.clone())))
            }
            GasEstimatorType::Alloy => {
                let estimator = ConfigurableGasPriceEstimator::new(
                    web3.provider.clone(),
                    EstimatorConfig {
                        past_blocks: default_past_blocks(),
                        reward_percentile: default_reward_percentile(),
                    },
                );
                estimators.push(Box::new(estimator))
            }
        }
    }
    anyhow::ensure!(
        !estimators.is_empty(),
        "all gas estimators failed to initialize"
    );
    Ok(PriorityGasPriceEstimating::new(estimators))
}

/// Extension trait for EIP-1559 gas price estimations.
pub trait Eip1559EstimationExt {
    /// Calculates the effective gas price that will be paid given the base fee.
    fn effective(self, base_fee: u64) -> u128;

    /// Scales fees by a multiplier in parts per thousand (e.g., 100 = +10%).
    fn scaled_by_pml(self, pml: u64) -> Self;
}

impl Eip1559EstimationExt for Eip1559Estimation {
    fn effective(self, base_fee: u64) -> u128 {
        calc_effective_gas_price(
            self.max_fee_per_gas,
            self.max_priority_fee_per_gas,
            Some(base_fee),
        )
    }

    fn scaled_by_pml(mut self, pml: u64) -> Self {
        self.max_fee_per_gas = {
            let n = self.max_fee_per_gas;
            n * (1000 + pml as u128) / 1000
        };
        self.max_priority_fee_per_gas = {
            let n = self.max_priority_fee_per_gas;
            n * (1000 + pml as u128) / 1000
        };
        self
    }
}
