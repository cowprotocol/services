//! Node-based gas estimation approach, queries the node for the current gas price.
//!
//! This approach is ported from the [`cowprotocol/gas-estimation`](https://github.com/cowprotocol/gas-estimation/tree/v0.7.3) crate's legacy estimation.

use {
    crate::gas_price_estimation::{GasPriceEstimating, price::GasPrice1559, u128_to_f64},
    alloy::providers::Provider,
    anyhow::{Context, Result},
    ethrpc::AlloyProvider,
};

/// Gas estimator that queries the node current gas price.
pub struct NodeGasPriceEstimator(AlloyProvider);

impl NodeGasPriceEstimator {
    pub fn new(provider: AlloyProvider) -> Self {
        Self(provider)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for NodeGasPriceEstimator {
    /// Returns the result of calling the `eth_gasPrice` endpoint as the gas estimation.
    async fn estimate(&self) -> Result<GasPrice1559> {
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
