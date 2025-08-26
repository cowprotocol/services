pub mod driver;
pub mod fake;

use {
    crate::{ethrpc::Web3, http_client::HttpClientFactory},
    anyhow::{Context, Result, anyhow, ensure},
    gas_estimation::{
        EthGasStation,
        GasNowGasStation,
        GasPriceEstimating,
        PriorityGasPriceEstimating,
        Transport,
        blocknative::BlockNative,
        nativegasestimator::NativeGasEstimator,
    },
    reqwest::header::{self, HeaderMap, HeaderValue},
    serde::de::DeserializeOwned,
    tracing::instrument,
    url::Url,
};
pub use {driver::DriverGasEstimator, fake::FakeGasPriceEstimator};

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum GasEstimatorType {
    EthGasStation,
    GasNow,
    Web3,
    BlockNative,
    Native,
    Driver,
}

#[derive(Clone)]
pub struct Client(pub reqwest::Client);

#[async_trait::async_trait]
impl Transport for Client {
    async fn get_json<T: DeserializeOwned>(&self, url: &str, header: HeaderMap) -> Result<T> {
        self.0
            .get(url)
            .headers(header)
            .send()
            .await
            .context("failed to make request")?
            .error_for_status()
            .context("response status is not success")?
            .json()
            .await
            .context("failed to decode response")
    }
}

#[instrument(skip_all)]
pub async fn create_priority_estimator(
    http_factory: &HttpClientFactory,
    web3: &Web3,
    estimator_types: &[GasEstimatorType],
    blocknative_api_key: Option<String>,
    driver_url: Option<Url>,
) -> Result<impl GasPriceEstimating + use<>> {
    let client = || Client(http_factory.create());
    let network_id = web3.eth().chain_id().await?.to_string();
    let mut estimators = Vec::<Box<dyn GasPriceEstimating>>::new();

    for estimator_type in estimator_types {
        tracing::info!("estimator {estimator_type:?}, networkid {network_id}");
        match estimator_type {
            GasEstimatorType::Driver => {
                let url = driver_url.clone().ok_or_else(|| {
                    anyhow!("Driver URL must be provided when using Driver gas estimator")
                })?;
                estimators.push(Box::new(DriverGasEstimator::new(
                    http_factory.create(),
                    url,
                )));
            }
            GasEstimatorType::BlockNative => {
                ensure!(is_mainnet(&network_id), "BlockNative only supports mainnet");
                ensure!(
                    blocknative_api_key.is_some(),
                    "BlockNative api key is empty"
                );
                let api_key = HeaderValue::from_str(&blocknative_api_key.clone().unwrap());
                let headers = match api_key {
                    Ok(mut api_key) => {
                        let mut headers = HeaderMap::new();
                        api_key.set_sensitive(true);
                        headers.insert(header::AUTHORIZATION, api_key);
                        headers
                    }
                    _ => HeaderMap::new(),
                };
                match BlockNative::new(client(), headers).await {
                    Ok(estimator) => estimators.push(Box::new(estimator)),
                    Err(err) => tracing::error!("blocknative failed: {}", err),
                }
            }
            GasEstimatorType::EthGasStation => {
                ensure!(
                    is_mainnet(&network_id),
                    "EthGasStation only supports mainnet"
                );
                estimators.push(Box::new(EthGasStation::new(client())))
            }
            GasEstimatorType::GasNow => {
                ensure!(is_mainnet(&network_id), "GasNow only supports mainnet");
                estimators.push(Box::new(GasNowGasStation::new(client())))
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

fn is_mainnet(network_id: &str) -> bool {
    network_id == "1"
}
