use crate::Web3;
use anyhow::{anyhow, Context, Result};
use gas_estimation::{
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
    }
}

#[derive(Clone)]
struct Client(reqwest::Client);

#[async_trait::async_trait]
impl Transport for Client {
    async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T> {
        self.0
            .get(url)
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
    client: &reqwest::Client,
    web3: &Web3,
    estimator_types: &[GasEstimatorType],
) -> Result<impl GasPriceEstimating> {
    let client = Client(client.clone());
    let network_id = web3.net().version().await?;
    let mut estimators = Vec::<Box<dyn GasPriceEstimating>>::new();
    for estimator_type in estimator_types {
        match estimator_type {
            GasEstimatorType::EthGasStation => {
                if !is_mainnet(&network_id) {
                    return Err(anyhow!("EthGasStation only supports mainnet"));
                }
                estimators.push(Box::new(EthGasStation::new(client.clone())))
            }
            GasEstimatorType::GasNow => {
                if !is_mainnet(&network_id) {
                    return Err(anyhow!("GasNow only supports mainnet"));
                }
                estimators.push(Box::new(GasNowGasStation::new(client.clone())))
            }
            GasEstimatorType::GnosisSafe => estimators.push(Box::new(
                GnosisSafeGasStation::with_network_id(&network_id, client.clone())?,
            )),
            GasEstimatorType::Web3 => estimators.push(Box::new(web3.clone())),
        }
    }
    Ok(PriorityGasPriceEstimating::new(estimators))
}

fn is_mainnet(network_id: &str) -> bool {
    network_id == "1"
}

pub struct FakeGasPriceEstimator(pub Arc<Mutex<f64>>);
#[async_trait::async_trait]
impl GasPriceEstimating for FakeGasPriceEstimator {
    async fn estimate_with_limits(&self, _: f64, _: std::time::Duration) -> Result<f64> {
        Ok(*self.0.lock().unwrap())
    }
}
