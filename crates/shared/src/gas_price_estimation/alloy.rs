//! Uses `alloy`'s logic for suggesting reasonable EIP-1559
//! gas price values. It computes the suggested gas price
//! based on the 20th percentile of fee rewards for
//! transactions of the last 10 blocks.
//! See <https://github.com/alloy-rs/alloy/blob/ad56bf0b974974179ee39daf694e400dba0f8ff7/crates/provider/src/utils.rs#L107-L122>
//! for the implementation details.

use {
    crate::gas_price_estimation::{GasPriceEstimating, price::GasPrice1559, u128_to_f64},
    alloy::providers::Provider,
    anyhow::{Context, Result},
    ethrpc::AlloyProvider,
    futures::TryFutureExt,
    tracing::instrument,
};

/// Estimates the gas price based on alloy's logic for computing a reasonable
/// EIP-1559 gas price.
pub struct Eip1559GasPriceEstimator(AlloyProvider);

impl Eip1559GasPriceEstimator {
    pub fn new(provider: AlloyProvider) -> Self {
        Self(provider)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for Eip1559GasPriceEstimator {
    /// Returns alloy's estimation for the EIP-1559 gas price.
    #[instrument(skip(self))]
    async fn estimate(&self) -> Result<crate::gas_price_estimation::price::GasPrice1559> {
        let fees = self
            .0
            .estimate_eip1559_fees()
            .map_err(|err| anyhow::anyhow!("could not estimate EIP 1559 fees: {err:?}"))
            .await?;

        let max_fee_per_gas = u128_to_f64(fees.max_fee_per_gas)
            .context("could not convert max_fee_per_gas to f64")?;

        Ok(GasPrice1559 {
            // We reuse `max_fee_per_gas` since the base fee only actually
            // exists in a mined block. For price estimates used to configure
            // the gas price of a transaction the base fee doesn't matter.
            base_fee_per_gas: max_fee_per_gas,
            max_fee_per_gas,
            max_priority_fee_per_gas: u128_to_f64(fees.max_priority_fee_per_gas)
                .context("could not convert max_priority_fee_per_gas to f64")?,
        })
    }
}
