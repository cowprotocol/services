pub mod alloy;
pub mod driver;
pub mod eth_node;
pub mod fake;
pub mod priority;

use {
    crate::{
        ethrpc::Web3,
        gas_price_estimation::{
            alloy::Eip1559GasPriceEstimator,
            eth_node::NodeGasPriceEstimator,
            priority::PriorityGasPriceEstimating,
        },
        http_client::HttpClientFactory,
    },
    ::alloy::{
        eips::eip1559::{Eip1559Estimation, calc_effective_gas_price},
        providers::Provider,
    },
    anyhow::Result,
    std::str::FromStr,
    tracing::instrument,
    url::Url,
};
pub use {driver::DriverGasEstimator, fake::FakeGasPriceEstimator};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait GasPriceEstimating: Send + Sync {
    /// Estimate the gas price for a transaction to be mined "quickly".
    async fn estimate(&self) -> Result<Eip1559Estimation>;
}

#[derive(Clone, Debug)]
pub enum GasEstimatorType {
    Web3,
    Driver(Url),
    Alloy,
}

impl FromStr for GasEstimatorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "web3" => Ok(GasEstimatorType::Web3),
            "alloy" => Ok(GasEstimatorType::Alloy),
            _ => Url::parse(s).map(GasEstimatorType::Driver).map_err(|e| {
                format!("expected 'web3', 'alloy', or a valid driver URL; got {s:?}: {e}")
            }),
        }
    }
}

#[instrument(skip_all)]
pub async fn create_priority_estimator(
    http_factory: &HttpClientFactory,
    web3: &Web3,
    estimator_types: &[GasEstimatorType],
) -> Result<impl GasPriceEstimating + use<>> {
    let network_id = web3.alloy.get_chain_id().await?.to_string();
    let mut estimators = Vec::<Box<dyn GasPriceEstimating>>::new();

    for estimator_type in estimator_types {
        tracing::info!("estimator {estimator_type:?}, networkid {network_id}");
        match estimator_type {
            GasEstimatorType::Driver(url) => {
                estimators.push(Box::new(DriverGasEstimator::new(
                    http_factory.create(),
                    url.clone(),
                )));
            }
            GasEstimatorType::Web3 => {
                estimators.push(Box::new(NodeGasPriceEstimator::new(web3.alloy.clone())))
            }
            GasEstimatorType::Alloy => {
                estimators.push(Box::new(Eip1559GasPriceEstimator::new(web3.alloy.clone())))
            }
        }
    }
    anyhow::ensure!(
        !estimators.is_empty(),
        "all gas estimators failed to initialize"
    );
    Ok(PriorityGasPriceEstimating::new(estimators))
}

pub trait Eip1559EstimationExt {
    fn effective(self, base_fee: Option<u64>) -> u128;
}

impl Eip1559EstimationExt for Eip1559Estimation {
    fn effective(self, base_fee: Option<u64>) -> u128 {
        calc_effective_gas_price(
            self.max_fee_per_gas,
            self.max_priority_fee_per_gas,
            base_fee,
        )
    }
}
