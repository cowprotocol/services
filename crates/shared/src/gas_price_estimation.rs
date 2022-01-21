use crate::Web3;
use anyhow::{ensure, Context, Result};
use gas_estimation::{
    blocknative::BlockNative, nativegasestimator::NativeGasEstimator, EstimatedGasPrice,
    EthGasStation, GasNowGasStation, GasPriceEstimating, GnosisSafeGasStation,
    PriorityGasPriceEstimating, Transport,
};
use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex};
use structopt::clap::arg_enum;

arg_enum! {
    #[derive(Debug)]
    pub enum GasEstimatorType {
        EthGasStation,
        GasNow,
        GnosisSafe,
        Web3,
        BlockNative,
        Native,
    }
}

#[derive(Clone)]
pub struct Client(pub reqwest::Client);

#[async_trait::async_trait]
impl Transport for Client {
    async fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        header: http::header::HeaderMap,
    ) -> Result<T> {
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
    client: reqwest::Client,
    web3: &Web3,
    estimator_types: &[GasEstimatorType],
    blocknative_api_key: Option<String>,
) -> Result<impl GasPriceEstimating> {
    let client = Client(client);
    let network_id = web3.net().version().await?;
    let mut estimators = Vec::<Box<dyn GasPriceEstimating>>::new();

    for estimator_type in estimator_types {
        match estimator_type {
            GasEstimatorType::BlockNative => {
                ensure!(is_mainnet(&network_id), "BlockNative only supports mainnet");
                ensure!(
                    blocknative_api_key.is_some(),
                    "BlockNative api key is empty"
                );
                let api_key =
                    http::header::HeaderValue::from_str(&blocknative_api_key.clone().unwrap());
                let headers = if let Ok(mut api_key) = api_key {
                    let mut headers = http::header::HeaderMap::new();
                    api_key.set_sensitive(true);
                    headers.insert(http::header::AUTHORIZATION, api_key);
                    headers
                } else {
                    http::header::HeaderMap::new()
                };
                match BlockNative::new(client.clone(), headers).await {
                    Ok(estimator) => estimators.push(Box::new(estimator)),
                    Err(err) => tracing::error!("blocknative failed: {}", err),
                }
            }
            GasEstimatorType::EthGasStation => {
                ensure!(
                    is_mainnet(&network_id),
                    "EthGasStation only supports mainnet"
                );
                estimators.push(Box::new(EthGasStation::new(client.clone())))
            }
            GasEstimatorType::GasNow => {
                ensure!(is_mainnet(&network_id), "GasNow only supports mainnet");
                estimators.push(Box::new(GasNowGasStation::new(client.clone())))
            }
            GasEstimatorType::GnosisSafe => estimators.push(Box::new(
                GnosisSafeGasStation::with_network_id(&network_id, client.clone())?,
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
    Ok(PriorityGasPriceEstimating::new(estimators))
}

fn is_mainnet(network_id: &str) -> bool {
    network_id == "1"
}

#[derive(Default)]
pub struct FakeGasPriceEstimator(pub Arc<Mutex<EstimatedGasPrice>>);

impl FakeGasPriceEstimator {
    pub fn new(gas_price: EstimatedGasPrice) -> Self {
        Self(Arc::new(Mutex::new(gas_price)))
    }
}
#[async_trait::async_trait]
impl GasPriceEstimating for FakeGasPriceEstimator {
    async fn estimate_with_limits(
        &self,
        _: f64,
        _: std::time::Duration,
    ) -> Result<EstimatedGasPrice> {
        Ok(*self.0.lock().unwrap())
    }
}
