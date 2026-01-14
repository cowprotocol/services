//! Ethereum node `GasPriceEstimating` implementation.

use {
    crate::gas_price_estimation::GasPriceEstimating,
    alloy::{eips::eip1559::Eip1559Estimation, providers::Provider},
    anyhow::{Context, Result},
    ethrpc::AlloyProvider,
};

pub struct NodeGasPriceEstimator(AlloyProvider);

impl NodeGasPriceEstimator {
    pub fn new(provider: AlloyProvider) -> Self {
        Self(provider)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for NodeGasPriceEstimator {
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
}
