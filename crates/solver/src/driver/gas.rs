//! Gas price estimation used for settlement submission.

use {
    anyhow::{ensure, Result},
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    num::Integer,
    std::{sync::Arc, time::Duration},
};

pub struct Estimator {
    inner: Arc<dyn GasPriceEstimating>,
    max_fee_factor: f64,
    gas_price_cap: f64,
}

impl Estimator {
    /// Creates a new gas price estimator for the driver.
    ///
    /// This estimator computes a EIP-1159 gas price with an upfront maximum
    /// `max_fee_per_gas` that is allowed for any given run. This allows
    /// settlement simulation to know upfront the actual minimum balance that
    /// would be required by a solver account (since, as per EIP-1559, an
    /// account needs at least `max_fee_per_gas * gas_limit` for a transaction
    /// to be valid, regardless of the `effective_gas_price` the transaction is
    /// executed with). This way, we:
    /// - Don't need to over-estimate a `max_fee_per_gas` value to account for
    ///   gas spikes, meaning we won't disregard solvers with lower, but
    ///   sufficient balances
    /// - Will only ever chose a solver IIF it will have enough balance to
    ///   execute a settlement up until a `gas_price_cap`, preventing settlement
    ///   submissions being stopped part-way through because of insufficient
    ///   balance for executing a transaction
    ///
    /// # Panics
    ///
    /// This method panics if the `timing` parameter contains 0-durations or the
    /// run duration is too long relative to the block duration.
    pub fn new(inner: Arc<dyn GasPriceEstimating>, timing: Timing) -> Self {
        let max_fee_factor = MAX_BASE_GAS_FEE_INCREASE_PER_BLOCK
            .powi(timing.blocks_per_run() as _)
            .min(MAX_FEE_FACTOR);

        Self {
            inner,
            max_fee_factor,
            gas_price_cap: f64::INFINITY,
        }
    }

    /// Sets the gas price cap for the estimator.
    pub fn with_gas_price_cap(mut self, gas_price_cap: f64) -> Self {
        self.gas_price_cap = gas_price_cap;
        self
    }
}

#[async_trait::async_trait]
impl GasPriceEstimating for Estimator {
    async fn estimate_with_limits(
        &self,
        gas_limit: f64,
        time_limit: Duration,
    ) -> Result<GasPrice1559> {
        let mut estimate = self
            .inner
            .estimate_with_limits(gas_limit, time_limit)
            .await?;

        estimate.max_fee_per_gas =
            (estimate.max_fee_per_gas * self.max_fee_factor).min(self.gas_price_cap);
        estimate = estimate.ceil();

        ensure!(estimate.is_valid(), "invalid gas estimate {estimate:?}");
        Ok(estimate)
    }
}

/// Timing configuration used by the estimator.
pub struct Timing {
    /// The expected block duration.
    pub block: Duration,
    /// The maximum driver run-loop duration.
    pub run: Duration,
}

/// The maximum amount the base gas fee can increase from one block to the
/// other.
///
/// This is derived from [EIP-1559](https://github.com/ethereum/EIPs/blob/master/EIPS/eip-1559.md):
/// ```text
/// BASE_FEE_MAX_CHANGE_DENOMINATOR = 8
/// base_fee_per_gas_delta = max(parent_base_fee_per_gas * gas_used_delta // parent_gas_target // BASE_FEE_MAX_CHANGE_DENOMINATOR, 1)
/// ```
///
/// Because the elasticity factor is 2, this means that the highest possible
/// `gas_used_delta == parent_gas_target`. Therefore, the highest possible
/// `base_fee_per_gas_delta` is `parent_base_fee_per_gas / 8`.
///
/// Example of this in action:
/// [Block 12998225](https://etherscan.io/block/12998225) with base fee of `43.353224173` and ~100% over the gas target.
/// Next [block 12998226](https://etherscan.io/block/12998226) has base fee of `48.771904644` which is an increase of ~12.5%.
const MAX_BASE_GAS_FEE_INCREASE_PER_BLOCK: f64 = 1.125;

/// The maximum `max_fee_factor` to use. This is to prevent exceedingly high
/// `max_fee_factor` values on networks with very short block intervals (like
/// on Gnosis Chain, which would require a factor of 30 for the currently
/// configured solution timing values.
const MAX_FEE_FACTOR: f64 = 5.;

impl Timing {
    fn blocks_per_run(&self) -> u16 {
        assert!(
            !self.block.is_zero() && !self.run.is_zero(),
            "zero duration",
        );

        Integer::div_ceil(&self.run.as_millis(), &self.block.as_millis())
            .try_into()
            .expect("overflow computing number of blocks for base fee scaling")
    }
}

#[cfg(test)]
mod tests {
    use {super::*, shared::gas_price_estimation::FakeGasPriceEstimator};

    #[test]
    fn timing_computes_blocks_per_run() {
        assert_eq!(
            Timing {
                block: Duration::from_secs(15),
                run: Duration::from_secs(60),
            }
            .blocks_per_run(),
            4
        );
        assert_eq!(
            Timing {
                block: Duration::from_secs(12),
                run: Duration::from_secs(150),
            }
            .blocks_per_run(),
            13
        );
    }

    #[tokio::test]
    async fn scales_max_gas_price() {
        let estimator = Estimator::new(
            Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 200.,
                max_priority_fee_per_gas: 10.,
            })),
            Timing {
                block: Duration::from_secs(15),
                run: Duration::from_secs(60),
            },
        );

        assert_eq!(
            estimator
                .estimate_with_limits(Default::default(), Default::default())
                .await
                .unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 321.,
                max_priority_fee_per_gas: 10.,
            }
        );
    }

    #[tokio::test]
    async fn capped_gas_price() {
        let estimator = Estimator::new(
            Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 200.,
                max_priority_fee_per_gas: 10.,
            })),
            Timing {
                block: Duration::from_secs(15),
                run: Duration::from_secs(60),
            },
        )
        .with_gas_price_cap(250.);

        assert_eq!(
            estimator
                .estimate_with_limits(Default::default(), Default::default())
                .await
                .unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 250.,
                max_priority_fee_per_gas: 10.,
            }
        );

        let estimator = estimator.with_gas_price_cap(150.);
        assert_eq!(
            estimator
                .estimate_with_limits(Default::default(), Default::default())
                .await
                .unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 100.,
                max_fee_per_gas: 150.,
                max_priority_fee_per_gas: 10.,
            }
        );

        let estimator = estimator.with_gas_price_cap(99.);
        assert!(estimator
            .estimate_with_limits(Default::default(), Default::default())
            .await
            .is_err());
    }
}
