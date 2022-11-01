use crate::{ethrpc::Web3, http_client::HttpClientFactory};
use anyhow::{ensure, Context, Result};
use gas_estimation::{
    blocknative::BlockNative, nativegasestimator::NativeGasEstimator, EthGasStation,
    GasNowGasStation, GasPrice1559, GasPriceEstimating, GnosisSafeGasStation,
    PriorityGasPriceEstimating, Transport,
};
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex};

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum GasEstimatorType {
    EthGasStation,
    GasNow,
    GnosisSafe,
    Web3,
    BlockNative,
    Native,
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

pub async fn create_priority_estimator(
    http_factory: &HttpClientFactory,
    web3: &Web3,
    estimator_types: &[GasEstimatorType],
    blocknative_api_key: Option<String>,
) -> Result<impl GasPriceEstimating> {
    let client = || Client(http_factory.create());
    let network_id = web3.net().version().await?;
    let mut estimators = Vec::<Box<dyn GasPriceEstimating>>::new();

    for estimator_type in estimator_types {
        tracing::info!("estimator {estimator_type:?}, networkid {network_id}");
        match estimator_type {
            GasEstimatorType::BlockNative => {
                ensure!(is_mainnet(&network_id), "BlockNative only supports mainnet");
                ensure!(
                    blocknative_api_key.is_some(),
                    "BlockNative api key is empty"
                );
                let api_key = HeaderValue::from_str(&blocknative_api_key.clone().unwrap());
                let headers = if let Ok(mut api_key) = api_key {
                    let mut headers = HeaderMap::new();
                    api_key.set_sensitive(true);
                    headers.insert(header::AUTHORIZATION, api_key);
                    headers
                } else {
                    HeaderMap::new()
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
            GasEstimatorType::GnosisSafe => estimators.push(Box::new(
                GnosisSafeGasStation::with_network_id(&network_id, client())?,
            )),
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

pub fn is_mainnet(network_id: &str) -> bool {
    network_id == "1"
}

#[derive(Default)]
pub struct FakeGasPriceEstimator(pub Arc<Mutex<GasPrice1559>>);

impl FakeGasPriceEstimator {
    pub fn new(gas_price: GasPrice1559) -> Self {
        Self(Arc::new(Mutex::new(gas_price)))
    }
}
#[async_trait::async_trait]
impl GasPriceEstimating for FakeGasPriceEstimator {
    async fn estimate_with_limits(&self, _: f64, _: std::time::Duration) -> Result<GasPrice1559> {
        Ok(*self.0.lock().unwrap())
    }
}
