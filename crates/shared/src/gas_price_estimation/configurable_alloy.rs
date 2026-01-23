//! Configurable EIP-1559 gas price estimator.
//!
//! Unlike alloy's default estimator which uses hardcoded values (10 blocks,
//! 20th percentile), this estimator allows configuring:
//! - Number of blocks to look back
//! - Reward percentile to use

use {
    crate::gas_price_estimation::{GasPriceEstimating, price::GasPrice1559, u128_to_f64},
    alloy::{
        eips::BlockNumberOrTag,
        providers::{Provider, utils::eip1559_default_estimator},
    },
    anyhow::{Context, Result},
    ethrpc::AlloyProvider,
    tracing::instrument,
};

#[derive(Debug, Clone, Copy)]
pub struct EstimatorConfig {
    /// Number of blocks to look back for fee history
    pub past_blocks: u64,
    /// Percentile of rewards to use for priority fee estimation
    pub reward_percentile: f64,
}

impl Default for EstimatorConfig {
    fn default() -> Self {
        Self {
            past_blocks: 10,
            reward_percentile: 20.0,
        }
    }
}

/// A configurable EIP-1559 gas price estimator.
///
/// Uses alloy's default estimation algorithm but with configurable
/// `past_blocks` and `reward_percentile` parameters for the fee history query.
pub struct ConfigurableGasPriceEstimator {
    provider: AlloyProvider,
    config: EstimatorConfig,
}

impl ConfigurableGasPriceEstimator {
    pub fn new(provider: AlloyProvider, config: EstimatorConfig) -> Self {
        Self { provider, config }
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for ConfigurableGasPriceEstimator {
    #[instrument(skip(self), fields(
        past_blocks = %self.config.past_blocks,
        reward_percentile = %self.config.reward_percentile
    ))]
    async fn estimate(&self) -> Result<GasPrice1559> {
        // Fetch fee history with our configured parameters
        let fee_history = self
            .provider
            .get_fee_history(
                self.config.past_blocks,
                BlockNumberOrTag::Latest,
                &[self.config.reward_percentile],
            )
            .await
            .context("failed to fetch fee history")?;

        // Get base fee: use latest block's base fee, or fall back to predicted
        // next block base fee if unavailable
        let base_fee_per_gas = match fee_history.latest_block_base_fee() {
            Some(base_fee) if base_fee != 0 => base_fee,
            _ => fee_history.base_fee_per_gas.last().copied().unwrap_or(0),
        };

        // Use alloy's default estimation algorithm
        let estimation =
            eip1559_default_estimator(base_fee_per_gas, &fee_history.reward.unwrap_or_default());

        Ok(GasPrice1559 {
            base_fee_per_gas: u128_to_f64(base_fee_per_gas)
                .context("could not convert base_fee_per_gas to f64")?,
            max_fee_per_gas: u128_to_f64(estimation.max_fee_per_gas)
                .context("could not convert max_fee_per_gas to f64")?,
            max_priority_fee_per_gas: u128_to_f64(estimation.max_priority_fee_per_gas)
                .context("could not convert max_priority_fee_per_gas to f64")?,
        })
    }
}
