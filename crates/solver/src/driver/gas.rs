//! Gas price estimation used for settlement submission.

use {
    anyhow::{ensure, Result},
    gas_estimation::{GasPrice1559, GasPriceEstimating},
    std::{sync::Arc, time::Duration},
};

pub struct Estimator {
    inner: Arc<dyn GasPriceEstimating>,
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
    /// - Will only ever chose a solver IFF it will have enough balance to
    ///   execute a settlement up until `max_fee_per_gas`, preventing settlement
    ///   submissions being stopped part-way through because of insufficient
    ///   balance for executing a transaction
    pub fn new(inner: Arc<dyn GasPriceEstimating>) -> Self {
        Self {
            inner,
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

        estimate.max_fee_per_gas = (estimate.base_fee_per_gas * MAX_FEE_FACTOR)
            .max(estimate.base_fee_per_gas + estimate.max_priority_fee_per_gas)
            .min(self.gas_price_cap);
        estimate.max_priority_fee_per_gas = estimate
            .max_priority_fee_per_gas
            .min(estimate.max_fee_per_gas);
        estimate = estimate.ceil();

        ensure!(estimate.is_valid(), "invalid gas estimate {estimate}");
        Ok(estimate)
    }
}

/// The factor of `base_fee_per_gas` to use for the `max_fee_per_gas` for gas
/// price estimates. This is chosen to be the maximum increase in the
/// `base_fee_per_gas` possible over a period of 12 blocks (which roughly
/// corresponds to the deadline a solver has for mining a transaction on
/// Mainnet + solvers solving time).
const MAX_FEE_FACTOR: f64 = 4.2;

#[cfg(test)]
mod tests {
    use {super::*, shared::gas_price_estimation::FakeGasPriceEstimator};

    #[tokio::test]
    async fn scales_max_gas_price() {
        let estimator = Estimator::new(Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
            base_fee_per_gas: 10.,
            max_fee_per_gas: 20.,
            max_priority_fee_per_gas: 1.,
        })));

        assert_eq!(
            estimator
                .estimate_with_limits(Default::default(), Default::default())
                .await
                .unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 10.,
                max_fee_per_gas: 42.,
                max_priority_fee_per_gas: 1.,
            }
        );
    }

    #[tokio::test]
    async fn respects_max_priority_fee() {
        let estimator = Estimator::new(Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
            base_fee_per_gas: 1.,
            max_fee_per_gas: 200.,
            max_priority_fee_per_gas: 99.,
        })));

        assert_eq!(
            estimator
                .estimate_with_limits(Default::default(), Default::default())
                .await
                .unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 1.,
                max_fee_per_gas: 100.,
                max_priority_fee_per_gas: 99.,
            }
        );

        let estimator = estimator.with_gas_price_cap(50.);

        assert_eq!(
            estimator
                .estimate_with_limits(Default::default(), Default::default())
                .await
                .unwrap(),
            GasPrice1559 {
                base_fee_per_gas: 1.,
                max_fee_per_gas: 50.,
                max_priority_fee_per_gas: 50.,
            }
        );
    }

    #[tokio::test]
    async fn capped_gas_price() {
        let estimator = Estimator::new(Arc::new(FakeGasPriceEstimator::new(GasPrice1559 {
            base_fee_per_gas: 100.,
            max_fee_per_gas: 200.,
            max_priority_fee_per_gas: 10.,
        })))
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
