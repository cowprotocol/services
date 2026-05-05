//! Slippage tolerance computation for DEX swaps.

use {
    crate::domain::{auction, dex::shared, eth},
    alloy::primitives::U256,
    bigdecimal::{BigDecimal, One, ToPrimitive, Zero},
    std::cmp,
};

/// DEX swap slippage limits.
#[derive(Clone, Debug)]
pub struct SlippageLimits {
    /// The relative slippage (percent) allowed for swaps.
    relative: BigDecimal,
    /// The maximum absolute slippage allowed for swaps.
    absolute: Option<eth::Ether>,
}

impl SlippageLimits {
    /// Creates a new slippage limits configuration.
    pub fn new(relative: BigDecimal, absolute: Option<eth::Ether>) -> Result<Self, anyhow::Error> {
        anyhow::ensure!(
            relative >= BigDecimal::zero() && relative <= BigDecimal::one(),
            "slippage relative tolerance must be in the range [0, 1]"
        );
        Ok(Self { relative, absolute })
    }

    /// Returns the slippage for the specified token amount.
    pub fn relative(&self, asset: &eth::Asset, tokens: &auction::Tokens) -> Slippage {
        let absolute_as_relative = shared::absolute_to_relative(self.absolute, asset, tokens);

        Slippage::new(cmp::min(
            self.relative.clone(),
            absolute_as_relative.unwrap_or(BigDecimal::one()),
        ))
    }
}

/// A relative slippage tolerance.
#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct Slippage(BigDecimal);

impl Slippage {
    /// Creates a new slippage from a decimal value.
    fn new(value: BigDecimal) -> Self {
        Self(value)
    }

    /// Returns 1% slippage.
    #[cfg(test)]
    pub fn one_percent() -> Self {
        Self::new("0.01".parse().unwrap())
    }

    /// Returns a zero slippage.
    pub fn zero() -> Self {
        Self::new(BigDecimal::zero())
    }

    /// Adds slippage to the specified amount.
    pub fn add(&self, amount: U256) -> U256 {
        let tolerance_amount = shared::compute_absolute_tolerance(amount, &self.0);
        amount.saturating_add(tolerance_amount)
    }

    /// Subtracts slippage from the specified amount.
    pub fn sub(&self, amount: U256) -> U256 {
        let tolerance_amount = shared::compute_absolute_tolerance(amount, &self.0);
        amount.saturating_sub(tolerance_amount)
    }

    /// Returns the slippage as a decimal factor.
    pub fn as_factor(&self) -> &BigDecimal {
        &self.0
    }

    /// Converts the slippage to basis points.
    pub fn as_bps(&self) -> Option<u16> {
        let bps = &self.0 * BigDecimal::from(10_000);
        bps.to_u32().and_then(|v| v.try_into().ok())
    }

    /// Returns the slippage as a percentage. e.g. 0.01 → 1.0.
    pub fn as_percent(&self) -> Option<f64> {
        (&self.0 * BigDecimal::from(100)).to_f64()
    }

    /// Rounds the slippage to the specified number of decimal places.
    pub fn round(&self, decimals: i64) -> Self {
        Self(self.0.round(decimals))
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
    fn slippage_tolerance() {
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
                // COW
                (
                    token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    price("0.000057"),
                ),
            ]
            .into_iter()
            .collect(),
        );
        let slippage = SlippageLimits::new(
            "0.01".parse().unwrap(), // 1%
            Some(ether("0.02")),
        )
        .unwrap();

        for (asset, relative, min, max) in [
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: U256::from(1_000_000_000_000_000_000_u128),
                },
                "0.01",
                990_000_000_000_000_000_u128,
                1_010_000_000_000_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: U256::from(100_000_000_000_000_000_000_u128),
                },
                "0.0002",
                99_980_000_000_000_000_000_u128,
                100_020_000_000_000_000_000_u128,
            ),
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: U256::from(1_000_000_000_u128), // 1K USDC
                },
                "0.01",
                990_000_000_u128,
                1_010_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            // 0.02 WETH <=> 33.91 USDC, and ~0.0033910778% of 1M
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: U256::from(1_000_000_000_000_u128), // 1M USDC
                },
                "0.000033911",
                999_966_089_222_u128,
                1_000_033_910_778_u128,
            ),
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    amount: U256::from(1_000_000_000_000_000_000_000_u128), // 1K COW
                },
                "0.01",
                990_000_000_000_000_000_000_u128,
                1_010_000_000_000_000_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            // 0.02 WETH <=> 350.88 COW, and ~0.0350877192982456140351% of 1M
            (
                eth::Asset {
                    token: token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    amount: U256::from(1_000_000_000_000_000_000_000_000_u128), // 1M COW
                },
                "0.000350877",
                999_649_122_807_017_543_859_649_u128,
                1_000_350_877_192_982_456_140_351_u128,
            ),
        ] {
            let relative = Slippage::new(relative.parse().unwrap());
            let min = U256::from(min);
            let max = U256::from(max);

            let computed = slippage.relative(&asset, &tokens);

            assert_eq!(computed.round(9), relative);
            assert_eq!(computed.sub(asset.amount), min);
            assert_eq!(computed.add(asset.amount), max);
        }
    }

    #[test]
    fn round_does_not_panic() {
        let slippage = Slippage::new(
            "42.115792089237316195423570985008687907853269984665640564039457584007913129639935"
                .parse()
                .unwrap(),
        );

        assert_eq!(slippage.round(4), Slippage::new("42.1158".parse().unwrap()));
    }

    #[test]
    fn handles_zero_amount_without_panic() {
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
            [(
                token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                price("1.0"),
            )]
            .into_iter()
            .collect(),
        );

        let slippage = SlippageLimits::new(
            "0.01".parse().unwrap(), // 1%
            Some(ether("0.02")),
        )
        .unwrap();

        // Test with zero amount - should not panic and use relative slippage fallback
        let asset_with_zero_amount = eth::Asset {
            token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            amount: U256::ZERO, // Zero amount
        };

        // This should not panic and should return the relative slippage (1%)
        let computed = slippage.relative(&asset_with_zero_amount, &tokens);
        assert_eq!(computed.as_factor(), &"0.01".parse::<BigDecimal>().unwrap());
    }
}
