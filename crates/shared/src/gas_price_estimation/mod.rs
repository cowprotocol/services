pub mod driver;
pub mod fake;

use {
    crate::{ethrpc::Web3, http_client::HttpClientFactory},
    anyhow::Result,
    gas_estimation::{
        GasPriceEstimating,
        PriorityGasPriceEstimating,
        nativegasestimator::NativeGasEstimator,
    },
    std::str::FromStr,
    tracing::instrument,
    url::Url,
};
pub use {driver::DriverGasEstimator, fake::FakeGasPriceEstimator};

#[derive(Clone, Debug)]
pub enum GasEstimatorType {
    Web3,
    Native,
    Driver(Url),
}

impl FromStr for GasEstimatorType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "web3" => Ok(GasEstimatorType::Web3),
            "native" => Ok(GasEstimatorType::Native),
            _ => Url::parse(s).map(GasEstimatorType::Driver).map_err(|e| {
                format!("expected 'web3', 'native', or a valid driver URL; got {s:?}: {e}")
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
    let network_id = web3.eth().chain_id().await?.to_string();
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
            GasEstimatorType::Web3 => estimators.push(Box::new(web3.clone())),
            GasEstimatorType::Native => {
                match NativeGasEstimator::new(web3.transport().clone(), None).await {
                    Ok(estimator) => estimators.push(Box::new(estimator)),
                    Err(err) => tracing::error!("nativegasestimator failed: {}", err),
                }
            }
        }
    }
    anyhow::ensure!(
        !estimators.is_empty(),
        "all gas estimators failed to initialize"
    );
    Ok(PriorityGasPriceEstimating::new(estimators))
}
