// Vendored implementation of GasPrice1559 to start removing the dependency on
// the gas_estimation crate
use serde::Serialize;
use {alloy::eips::eip1559::calc_effective_gas_price, num::Zero};

/// EIP1559 gas price
#[derive(Debug, Default, Clone, Copy, PartialEq, PartialOrd, Serialize)]
pub struct GasPrice1559 {
    // Estimated base fee for the pending block (block currently being mined)
    pub base_fee_per_gas: f64,
    // Maximum gas price willing to pay for the transaction.
    pub max_fee_per_gas: f64,
    // Priority fee used to incentivize miners to include the tx in case of network congestion.
    pub max_priority_fee_per_gas: f64,
}

impl GasPrice1559 {
    pub fn effective_gas_price(&self) -> f64 {
        calc_effective_gas_price(
            self.max_fee_per_gas as u128,
            self.max_priority_fee_per_gas as u128,
            if self.base_fee_per_gas.is_zero() {
                None
            } else {
                Some(self.base_fee_per_gas as u64)
            },
        ) as f64
    }

    // Bump gas price by factor.
    pub fn bump(self, factor: f64) -> Self {
        Self {
            max_fee_per_gas: self.max_fee_per_gas * factor,
            max_priority_fee_per_gas: self.max_priority_fee_per_gas * factor,
            ..self
        }
    }

    // Ceil gas price (since its defined as float).
    pub fn ceil(self) -> Self {
        Self {
            max_fee_per_gas: self.max_fee_per_gas.ceil(),
            max_priority_fee_per_gas: self.max_priority_fee_per_gas.ceil(),
            ..self
        }
    }

    // If current cap if higher then the input, set to input.
    pub fn limit_cap(self, cap: f64) -> Self {
        Self {
            max_fee_per_gas: self.max_fee_per_gas.min(cap),
            max_priority_fee_per_gas: self
                .max_priority_fee_per_gas
                .min(self.max_fee_per_gas.min(cap)), /* enforce max_priority_fee_per_gas <=
                                                      * max_fee_per_gas */
            ..self
        }
    }
}

impl std::fmt::Display for GasPrice1559 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format_unit = |wei| {
            let gwei: f64 = wei / 1e9;
            if gwei >= 1.0 {
                format!("{:.2} Gwei", gwei)
            } else {
                format!("{wei} wei")
            }
        };
        write!(
            f,
            "{{ max_fee: {}, max_priority_fee: {}, base_fee: {} }}",
            format_unit(self.max_fee_per_gas),
            format_unit(self.max_priority_fee_per_gas),
            format_unit(self.base_fee_per_gas),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::gas_price_estimation::price::GasPrice1559;

    // Copied from the source: https://github.com/ashleygwilliams/assert_approx_eq/blob/master/src/lib.rs
    // should be removed as we move away from expressing gas in f64
    macro_rules! assert_approx_eq {
        ($a:expr, $b:expr) => {{
            let eps = 1.0e-6;
            let (a, b) = (&$a, &$b);
            assert!(
                (*a - *b).abs() < eps,
                "assertion failed: `(left !== right)` (left: `{:?}`, right: `{:?}`, expect diff: \
                 `{:?}`, real diff: `{:?}`)",
                *a,
                *b,
                eps,
                (*a - *b).abs()
            );
        }};
    }

    #[test]
    fn bump_and_ceil() {
        let gas_price = GasPrice1559 {
            max_fee_per_gas: 2.0,
            max_priority_fee_per_gas: 3.0,
            ..Default::default()
        };

        let gas_price_bumped = GasPrice1559 {
            max_fee_per_gas: 2.25,
            max_priority_fee_per_gas: 3.375,
            ..Default::default()
        };

        let gas_price_bumped_and_ceiled = GasPrice1559 {
            max_fee_per_gas: 3.0,
            max_priority_fee_per_gas: 4.0,
            ..Default::default()
        };

        assert_eq!(gas_price.bump(1.125), gas_price_bumped);
        assert_eq!(gas_price.bump(1.125).ceil(), gas_price_bumped_and_ceiled);
    }

    #[test]
    fn limit_cap_only_max_fee_capped() {
        let gas_price = GasPrice1559 {
            max_fee_per_gas: 5.0,
            max_priority_fee_per_gas: 3.0,
            ..Default::default()
        };

        let gas_price_capped = GasPrice1559 {
            max_fee_per_gas: 4.0,
            max_priority_fee_per_gas: 3.0,
            ..Default::default()
        };

        assert_eq!(gas_price.limit_cap(4.0), gas_price_capped);
    }

    #[test]
    fn limit_cap_max_fee_and_max_priority_capped() {
        let gas_price = GasPrice1559 {
            max_fee_per_gas: 5.0,
            max_priority_fee_per_gas: 3.0,
            ..Default::default()
        };

        let gas_price_capped = GasPrice1559 {
            max_fee_per_gas: 2.0,
            max_priority_fee_per_gas: 2.0,
            ..Default::default()
        };

        assert_eq!(gas_price.limit_cap(2.0), gas_price_capped);
    }

    #[test]
    fn estimate_eip1559() {
        assert_approx_eq!(
            GasPrice1559 {
                max_fee_per_gas: 10.0,
                max_priority_fee_per_gas: 5.0,
                base_fee_per_gas: 2.0
            }
            .effective_gas_price(),
            7.0
        );

        assert_approx_eq!(
            GasPrice1559 {
                max_fee_per_gas: 10.0,
                max_priority_fee_per_gas: 8.0,
                base_fee_per_gas: 2.0
            }
            .effective_gas_price(),
            10.0
        );

        assert_approx_eq!(
            GasPrice1559 {
                max_fee_per_gas: 10.0,
                max_priority_fee_per_gas: 10.0,
                base_fee_per_gas: 2.0
            }
            .effective_gas_price(),
            10.0
        );
    }
}
