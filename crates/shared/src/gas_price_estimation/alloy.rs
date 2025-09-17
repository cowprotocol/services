//! Uses `alloy`'s logic for suggesting reasonable EIP-1559
//! gas price values. It computes the suggested gas price
//! based on the 20th percentile of fee rewards for
//! transactions of the last 10 blocks.
//! See <https://github.com/alloy-rs/alloy/blob/ad56bf0b974974179ee39daf694e400dba0f8ff7/crates/provider/src/utils.rs#L107-L122>
//! for the implementation details.

use {
    alloy::{consensus::BlockHeader, providers::Provider},
    anyhow::{Context, Result},
    ethrpc::AlloyProvider,
    futures::TryFutureExt,
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    std::{ops::Mul, time::Duration},
    tracing::instrument,
};

pub struct AlloyGasPriceEstimator(AlloyProvider);

impl AlloyGasPriceEstimator {
    pub fn new(provider: AlloyProvider) -> Self {
        Self(provider)
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for AlloyGasPriceEstimator {
    #[instrument(skip(self))]
    async fn estimate_with_limits(
        &self,
        _gas_limit: f64,
        _time_limit: Duration,
    ) -> Result<GasPrice1559> {
        let estimate_fees = self
            .0
            .estimate_eip1559_fees()
            .map_err(|err| anyhow::anyhow!("could not estimate EIP 1559 fees: {err:?}"));
        let get_block = self
            .0
            .get_block(alloy::eips::BlockId::Number(
                alloy::eips::BlockNumberOrTag::Latest,
            ))
            .into_future()
            .map_err(|err| anyhow::anyhow!("could not fetch latest block: {err:?}"));
        let (fees, block) = tokio::try_join!(estimate_fees, get_block)?;

        /// `Alloy`'s constant growth factor to estimate the base_fee
        /// of the next block. (<https://github.com/alloy-rs/alloy/blob/ad56bf0b974974179ee39daf694e400dba0f8ff7/crates/provider/src/utils.rs#L19>)
        const MAX_GAS_PRICE_INCREASE_PER_BLOCK: f64 = 2.;

        let base_fee_per_gas = u64_to_f64(
            block
                .context("latest block is missing")?
                .into_consensus_header()
                .base_fee_per_gas()
                .context("no base_fee_per_gas")?,
        )
        .context("could not convert base_fee_per_gas to f64")?
        .mul(MAX_GAS_PRICE_INCREASE_PER_BLOCK);

        Ok(GasPrice1559 {
            base_fee_per_gas,
            max_fee_per_gas: u128_to_f64(fees.max_fee_per_gas)
                .context("could not convert max_fee_per_gas to f64")?,
            max_priority_fee_per_gas: u128_to_f64(fees.max_priority_fee_per_gas)
                .context("could not convert max_priority_fee_per_gas to f64")?,
        })
    }
}

fn u128_to_f64(val: u128) -> Result<f64> {
    if val > 2u128.pow(f64::MANTISSA_DIGITS) {
        anyhow::bail!(format!("could not convert u128 to f64: {val}"));
    }
    Ok(val as f64)
}

fn u64_to_f64(val: u64) -> Result<f64> {
    if val > 2u64.pow(f64::MANTISSA_DIGITS) {
        anyhow::bail!(format!("could not convert u64 to f64: {val}"));
    }
    Ok(val as f64)
}
