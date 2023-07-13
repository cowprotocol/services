//! Slippage tolerance computation for DEX swaps.

use {
    crate::{
        domain::{auction, eth},
        util::conv,
    },
    bigdecimal::BigDecimal,
    ethereum_types::U256,
    num::{BigUint, Integer, One, Zero},
    std::cmp,
};

/// DEX swap slippage limits. The actual slippage used for a swap is bounded by
/// a relative amount, and an absolute Ether value. These limits are used to
/// determine the actual relative slippage to use for a particular asset (i.e.
/// token and amount).
#[derive(Clone, Debug)]
pub struct Limits {
    relative: BigDecimal,
    absolute: Option<eth::Ether>,
}

impl Limits {
    /// Creates a new [`Limits`] instance. Returns `None` if the `relative`
    /// slippage limit outside the valid range of [0, 1].
    pub fn new(relative: BigDecimal, absolute: Option<eth::Ether>) -> Option<Self> {
        (relative >= Zero::zero() && relative <= One::one()).then_some(Self { relative, absolute })
    }

    /// Computes the actual slippage tolerance to use for an asset using the
    /// specified reference prices.
    pub fn relative(&self, asset: &eth::Asset, tokens: &auction::Tokens) -> Slippage {
        if let (Some(absolute), Some(price)) =
            (&self.absolute, tokens.reference_price(&asset.token))
        {
            let absolute = conv::ether_to_decimal(absolute);
            let amount = conv::ether_to_decimal(&eth::Ether(asset.amount))
                * conv::ether_to_decimal(&price.0);

            let max_relative = absolute / amount;
            let tolerance = cmp::min(max_relative, self.relative.clone());

            Slippage(tolerance)
        } else {
            Slippage(self.relative.clone())
        }
    }
}

/// A relative slippage tolerance.
///
/// Relative slippage has saturating semantics. I.e. if adding slippage to a
/// token amount would overflow a `U256`, then `U256::max_value()` is returned
/// instead.
#[derive(Debug, Eq, PartialEq)]
pub struct Slippage(BigDecimal);

impl Slippage {
    pub fn one_percent() -> Self {
        Self("0.01".parse().unwrap())
    }

    /// Adds slippage to the specified token amount. This can be used to account
    /// for negative slippage in a sell amount.
    pub fn add(&self, amount: U256) -> U256 {
        amount.saturating_add(self.abs(&amount))
    }

    /// Subtracts slippage to the specified token amount. This can be used to
    /// account for negative slippage in a buy amount.
    pub fn sub(&self, amount: U256) -> U256 {
        amount.saturating_sub(self.abs(&amount))
    }

    /// Returns the absolute slippage amount.
    fn abs(&self, amount: &U256) -> U256 {
        let amount = conv::u256_to_biguint(amount);
        let (int, exp) = self.0.as_bigint_and_exponent();

        let numer = amount * int.to_biguint().expect("positive by construction");
        let denom = BigUint::from(10_u8).pow(exp.unsigned_abs().try_into().unwrap_or(u32::MAX));

        let abs = numer.div_ceil(&denom);
        conv::biguint_to_u256(&abs).unwrap_or_else(U256::max_value)
    }

    /// Returns the relative slippage as a `BigDecimal` factor.
    pub fn as_factor(&self) -> &BigDecimal {
        &self.0
    }

    /// Rounds a relative slippage value to the specified decimal precision.
    pub fn round(&self, arg: u32) -> Self {
        Self(self.0.round(arg as _))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::domain::auction};

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
        let slippage = Limits {
            relative: "0.01".parse().unwrap(), // 1%
            absolute: Some(ether("0.02")),
        };

        for (asset, relative, min, max) in [
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: 1_000_000_000_000_000_000_u128.into(),
                },
                "0.01",
                990_000_000_000_000_000,
                1_010_000_000_000_000_000,
            ),
            // tolerance capped by absolute slippage
            (
                eth::Asset {
                    token: token("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
                    amount: 100_000_000_000_000_000_000_u128.into(),
                },
                "0.0002",
                99_980_000_000_000_000_000,
                100_020_000_000_000_000_000,
            ),
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: 1_000_000_000_u128.into(), // 1K USDC
                },
                "0.01",
                990_000_000,
                1_010_000_000,
            ),
            // tolerance capped by absolute slippage
            // 0.02 WETH <=> 33.91 USDC, and ~0.0033910778% of 1M
            (
                eth::Asset {
                    token: token("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
                    amount: 1_000_000_000_000_u128.into(), // 1M USDC
                },
                "0.000033911",
                999_966_089_222,
                1_000_033_910_778,
            ),
            // tolerance defined by relative slippage
            (
                eth::Asset {
                    token: token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    amount: 1_000_000_000_000_000_000_000_u128.into(), // 1K COW
                },
                "0.01",
                990_000_000_000_000_000_000u128,
                1_010_000_000_000_000_000_000_u128,
            ),
            // tolerance capped by absolute slippage
            // 0.02 WETH <=> 350.88 COW, and ~0.0350877192982456140351% of 1M
            (
                eth::Asset {
                    token: token("0xDEf1CA1fb7FBcDC777520aa7f396b4E015F497aB"),
                    amount: 1_000_000_000_000_000_000_000_000_u128.into(), // 1M COW
                },
                "0.000350877",
                999_649_122_807_017_543_859_649,
                1_000_350_877_192_982_456_140_351,
            ),
        ] {
            let relative = Slippage(relative.parse().unwrap());
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
        let slippage = Slippage(
            "42.115792089237316195423570985008687907853269984665640564039457584007913129639935"
                .parse()
                .unwrap(),
        );

        assert_eq!(slippage.round(4), Slippage("42.1158".parse().unwrap()));
    }
}
