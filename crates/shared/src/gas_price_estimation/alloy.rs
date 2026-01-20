//! Uses `alloy`'s logic for suggesting reasonable EIP-1559
//! gas price values. It computes the suggested gas price
//! based on the 20th percentile of fee rewards for
//! transactions of the last 10 blocks.
//! See <https://github.com/alloy-rs/alloy/blob/ad56bf0b974974179ee39daf694e400dba0f8ff7/crates/provider/src/utils.rs#L107-L122>
//! for the implementation details.

use {
    crate::gas_price_estimation::GasPriceEstimating,
    alloy::{
        eips::{BlockId, eip1559::Eip1559Estimation},
        providers::Provider,
    },
    anyhow::{Result, anyhow},
    ethrpc::AlloyProvider,
    futures::TryFutureExt,
    tracing::instrument,
};

/// Estimates an EIP-1559 gas price based on the 20th percentile of fee rewards
/// for transactions of the last 10 blocks.
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
    async fn estimate(&self) -> Result<Eip1559Estimation> {
        let fees = self
            .0
            .estimate_eip1559_fees()
            .map_err(|err| anyhow::anyhow!("could not estimate EIP 1559 fees: {err:?}"))
            .await?;

        Ok(Eip1559Estimation {
            max_fee_per_gas: fees.max_fee_per_gas,
            max_priority_fee_per_gas: fees.max_priority_fee_per_gas,
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
