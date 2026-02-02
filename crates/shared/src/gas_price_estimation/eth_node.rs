//! Node-based gas estimation approach, queries the node for the current gas
//! price.
//!
//! This approach is ported from the [`cowprotocol/gas-estimation`](https://github.com/cowprotocol/gas-estimation/tree/v0.7.3) crate's legacy estimation.

use {
    crate::gas_price_estimation::GasPriceEstimating,
    alloy::{
        eips::{BlockId, eip1559::Eip1559Estimation},
        providers::Provider,
    },
    anyhow::{Context, Result, anyhow},
    ethrpc::AlloyProvider,
};

/// Gas estimator that queries the node current gas price using `eth_gasPrice`.
pub struct NodeGasPriceEstimator(AlloyProvider);

impl NodeGasPriceEstimator {
    pub fn new(provider: AlloyProvider) -> Self {
        Self(provider)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for NodeGasPriceEstimator {
    /// Returns the result of calling the `eth_gasPrice` endpoint as the gas
    /// estimation.
    async fn estimate(&self) -> Result<Eip1559Estimation> {
        let legacy = self
            .0
            .get_gas_price()
            .await
            .context("failed to get web3 gas price")?;

        Ok(Eip1559Estimation {
            max_fee_per_gas: legacy,
            max_priority_fee_per_gas: legacy,
        })
    }

    async fn base_fee(&self) -> Result<Option<u64>> {
        Ok(self
            .0
            .get_block(BlockId::latest())
            .await?
            .ok_or_else(|| anyhow!("fecthed block does not have header"))?
            .header
            .base_fee_per_gas)
    }
}
