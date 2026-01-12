//! Ethereum node `GasPriceEstimating` implementation.

use {
    crate::gas_price_estimation::{GasPriceEstimating, u128_to_f64},
    alloy::providers::Provider,
    anyhow::{Context, Result},
    ethrpc::AlloyProvider,
    crate::gas_price_estimation::price::GasPrice1559,
    std::time::Duration,
};

pub struct NodeGasPriceEstimator(AlloyProvider);

impl NodeGasPriceEstimator {
    pub fn new(provider: AlloyProvider) -> Self {
        Self(provider)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for NodeGasPriceEstimator {
    async fn estimate_with_limits(
        &self,
        _gas_limit: f64,
        _time_limit: Duration,
    ) -> Result<GasPrice1559> {
        let legacy = self
            .0
            .get_gas_price()
            .await
            .context("failed to get web3 gas price")
            .map(u128_to_f64)??;

        Ok(GasPrice1559 {
            base_fee_per_gas: 0.0,
            max_fee_per_gas: legacy,
            max_priority_fee_per_gas: legacy,
        })
    }
}
