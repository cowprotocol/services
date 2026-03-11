//! Minimum surplus requirements for DEX swaps.

use {
    crate::domain::{auction, dex::shared, eth},
    alloy::primitives::U256,
    bigdecimal::{BigDecimal, Zero},
    std::cmp,
};

/// DEX swap minimum surplus limits.
#[derive(Clone, Debug)]
pub struct MinimumSurplusLimits {
    /// The relative minimum surplus (percent) required for swaps.
    relative: BigDecimal,
    /// The absolute minimum surplus required for swaps.
    absolute: Option<eth::Ether>,
}

impl MinimumSurplusLimits {
    /// Creates a new minimum surplus limits configuration.
    pub fn new(relative: BigDecimal, absolute: Option<eth::Ether>) -> Result<Self, anyhow::Error> {
        anyhow::ensure!(
            relative >= BigDecimal::zero(),
            "minimum surplus relative tolerance must be non-negative"
        );
        Ok(Self { relative, absolute })
    }

    /// Returns the minimum surplus for the specified token amount.
    pub fn relative(&self, asset: &eth::Asset, tokens: &auction::Tokens) -> MinimumSurplus {
        let absolute_as_relative = shared::absolute_to_relative(self.absolute, asset, tokens);

        MinimumSurplus::new(cmp::max(
            self.relative.clone(),
            absolute_as_relative.unwrap_or(BigDecimal::zero()),
        ))
    }
}

/// A relative minimum surplus requirement.
#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct MinimumSurplus(BigDecimal);

impl MinimumSurplus {
    /// Creates a new minimum surplus from a decimal value.
    fn new(value: BigDecimal) -> Self {
        Self(value)
    }

    /// Adds minimum surplus to the specified amount.
    pub fn add(&self, amount: U256) -> U256 {
        let tolerance_amount = shared::compute_absolute_tolerance(amount, &self.0);
        amount.saturating_add(tolerance_amount)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            domain::{auction, eth},
            util::conv,
        },
    };

    #[test]
    fn minimum_surplus_requirement() {
        let token = |t: &str| eth::TokenAddress(t.parse().unwrap());
        let ether = |e: &str| conv::decimal_to_ether(&e.parse().unwrap()).unwrap();
        let price = |e: &str| auction::Token {
            decimals: Default::default(),
            symbol: Default::default(),
            reference_price: Some(auction::Price(ether(e))),
            available_balance: Default::default(),
            trusted: Default::default(),
        };

        let tokens = auction::Tokens(
            [
                // WETH
                (
                    token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    price("1.0"),
                ),
                // USDC
                (
                    token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    price("589783000.0"),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let minimum_surplus = MinimumSurplusLimits::new(
            "0.01".parse().unwrap(), // 1%
            Some(ether("0.02")),
        )
        .unwrap();

        for (asset, relative, min_buy) in [
            // Small amount: absolute minimum surplus dominates
            // 0.5 WETH * 0.02/0.5 = 0.02 WETH absolute = 4% > 1% relative
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: U256::from(500_000_000_000_000_000_u128), // 0.5 WETH
                },
                "0.04",
                520_000_000_000_000_000_u128,
            ),
            // Medium amount: relative minimum surplus dominates
            // 5 WETH * 1% = 0.05 WETH > 0.02 WETH absolute
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: U256::from(5_000_000_000_000_000_000_u128), // 5 WETH
                },
                "0.01",
                5_050_000_000_000_000_000_u128,
            ),
            // For USDC: relative dominates for this amount
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: U256::from(10_000_000_000_u128), // 10K USDC
                },
                "0.01",
                10_100_000_000_u128,
            ),
        ] {
            let relative = MinimumSurplus::new(relative.parse().unwrap());
            let min_buy = U256::from(min_buy);

            let computed = minimum_surplus.relative(&asset, &tokens);

            assert_eq!(computed, relative);
            assert_eq!(computed.add(asset.amount), min_buy);
        }
    }
}
