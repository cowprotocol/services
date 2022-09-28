//! Module defining slippage computation for AMM liquidiy.

use crate::settlement::external_prices::ExternalPrices;
use anyhow::{Context as _, Result};
use ethcontract::{H160, U256};
use num::{BigInt, BigRational, Integer as _, ToPrimitive as _};
use std::{borrow::Cow, cmp};

/// Constant maximum slippage of 10 BPS (0.1%) to use for on-chain liquidity.
pub const DEFAULT_MAX_SLIPPAGE_BPS: u32 = 10;

/// Basis points in 100%.
const BPS_BASE: u32 = 10000;

/// Component used for computing negative slippage limits for internal solvers.
#[derive(Clone, Debug)]
pub struct SlippageCalculator {
    /// The maximum relative slippage factor.
    relative: BigRational,
    /// The maximum absolute slippage in native tokens.
    absolute: Option<BigInt>,
}

impl SlippageCalculator {
    pub fn from_bps(relative_bps: u32, absolute: Option<U256>) -> Self {
        Self {
            relative: BigRational::new(relative_bps.into(), BPS_BASE.into()),
            absolute: absolute.map(|value| number_conversions::u256_to_big_int(&value)),
        }
    }

    pub fn compute(
        &self,
        external_prices: &ExternalPrices,
        token: H160,
        amount: U256,
    ) -> Result<SlippageAmount> {
        let price = external_prices
            .price(&token)
            .context("missing token price")?;

        let (relative, absolute) =
            self.compute_inner(price, number_conversions::u256_to_big_int(&amount));
        let slippage = SlippageAmount::from_num(&relative, &absolute)?;

        if *relative < self.relative {
            tracing::debug!(
                ?token,
                %amount,
                relative = ?slippage.relative,
                absolute = %slippage.absolute,
                "reducing relative to respect maximum absolute slippage",
            );
        }

        Ok(slippage)
    }

    pub fn compute_with_price(&self, price: &BigRational, amount: U256) -> Result<SlippageAmount> {
        let (relative, absolute) =
            self.compute_inner(price, number_conversions::u256_to_big_int(&amount));
        SlippageAmount::from_num(&relative, &absolute)
    }

    fn compute_inner(
        &self,
        token_price: &BigRational,
        amount: BigInt,
    ) -> (Cow<BigRational>, BigInt) {
        let relative = if let Some(max_absolute_native_token) = self.absolute.clone() {
            let max_absolute_slippage =
                BigRational::new(max_absolute_native_token, 1.into()) / token_price;

            let max_relative_slippage_respecting_absolute_limit = max_absolute_slippage / &amount;

            cmp::min(
                Cow::Owned(max_relative_slippage_respecting_absolute_limit),
                Cow::Borrowed(&self.relative),
            )
        } else {
            Cow::Borrowed(&self.relative)
        };
        let absolute = {
            let ratio = &*relative * amount;
            ratio.numer().div_ceil(ratio.denom())
        };

        (relative, absolute)
    }
}

impl Default for SlippageCalculator {
    fn default() -> Self {
        Self::from_bps(DEFAULT_MAX_SLIPPAGE_BPS, None)
    }
}

/// A result of a slippage computation containing both relative and absolute
/// slippage amounts.
#[derive(Clone, Copy, Debug, Default)]
pub struct SlippageAmount {
    /// The relative slippage amount factor.
    relative: f64,
    /// The absolute slippage amount in the token it was computed for.
    absolute: U256,
}

impl SlippageAmount {
    /// Computes a slippage amount from arbitrary precision `num` values.
    fn from_num(relative: &BigRational, absolute: &BigInt) -> Result<Self> {
        let relative = relative
            .to_f64()
            .context("relative slippage ratio is not a number")?;
        let absolute = number_conversions::big_int_to_u256(absolute)?;

        Ok(Self { relative, absolute })
    }

    /// Reduce the specified amount by the constant slippage.
    pub fn amount_minus(&self, amount: U256) -> U256 {
        amount.saturating_sub(self.absolute)
    }

    /// Increase the specified amount by the constant slippage.
    pub fn amount_plus(&self, amount: U256) -> U256 {
        amount.saturating_add(self.absolute)
    }

    /// Returns the relative slippage as a factor.
    pub fn as_factor(&self) -> f64 {
        self.relative
    }

    /// Returns the relative slippage as a percentage.
    pub fn as_percentage(&self) -> f64 {
        self.relative * 100.
    }

    /// Returns the relative slippage as basis points rounded down.
    pub fn as_bps(&self) -> u32 {
        (self.relative * 10000.) as _
    }
}

/// Multiply an integer amount by a rational, with additional handling in case
/// of overflows.
fn slippage_for_amount(amount: U256) -> SlippageAmount {
    let calculator = SlippageCalculator::default();
    let (relative, absolute) =
        calculator.compute_inner(&num::one(), number_conversions::u256_to_big_int(&amount));

    // The computed slippage amount for the default calculator will always
    // succeed because:
    // - The relative amount will be 0.1%, which is a known good value
    // - 0.1% of any U256 fits into a U256, so it will also be a good value
    SlippageAmount::from_num(&relative, &absolute).unwrap()
}

/// Reduce the specified amount by the constant slippage.
pub fn amount_minus_max_slippage(amount: U256) -> U256 {
    slippage_for_amount(amount).amount_minus(amount)
}

/// Increase the specified amount by the constant slippage.
pub fn amount_plus_max_slippage(amount: U256) -> U256 {
    slippage_for_amount(amount).amount_plus(amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::external_prices::externalprices;
    use shared::conversions::U256Ext as _;
    use testlib::tokens::{USDC, WETH};

    #[test]
    fn limits_max_slippage() {
        let calculator = SlippageCalculator::from_bps(10, Some(U256::exp10(17)));

        for (price, amount, expected_slippage) in [
            (U256::exp10(9).to_big_rational(), U256::exp10(12), 1),
            (BigRational::new(2.into(), 1000.into()), U256::exp10(23), 5),
            (U256::exp10(9).to_big_rational(), U256::exp10(8), 10),
            (U256::exp10(9).to_big_rational(), U256::exp10(17), 0),
        ] {
            let slippage = calculator
                .compute(
                    &externalprices! { native_token: WETH, USDC => price },
                    USDC,
                    amount,
                )
                .unwrap();

            assert_eq!(slippage.as_bps(), expected_slippage);
        }
    }

    #[test]
    fn errors_on_missing_token_price() {
        let calculator = SlippageCalculator::default();
        assert!(calculator
            .compute(
                &externalprices! { native_token: WETH, },
                USDC,
                1_000_000.into(),
            )
            .is_err());
    }

    #[test]
    fn test_out_amount_with_slippage() {
        assert_eq!(amount_minus_max_slippage(0.into()), 0.into());
        assert_eq!(amount_minus_max_slippage(100.into()), 99.into());
        assert_eq!(amount_minus_max_slippage(1000.into()), 999.into());
        assert_eq!(amount_minus_max_slippage(10000.into()), 9990.into());
        assert_eq!(amount_minus_max_slippage(10001.into()), 9990.into());
        assert_eq!(
            amount_minus_max_slippage(U256::MAX),
            U256::from_dec_str(
                "115676297148078879228147414023679219945416714680974923475418126423905216510295"
            )
            .unwrap()
        );
    }

    #[test]
    fn test_in_amount_with_slippage() {
        assert_eq!(amount_plus_max_slippage(0.into()), 0.into());
        assert_eq!(amount_plus_max_slippage(100.into()), 101.into());
        assert_eq!(amount_plus_max_slippage(1000.into()), 1001.into());
        assert_eq!(amount_plus_max_slippage(10000.into()), 10010.into());
        assert_eq!(amount_plus_max_slippage(10001.into()), 10012.into());
        assert_eq!(amount_plus_max_slippage(U256::MAX), U256::MAX);
    }
}
