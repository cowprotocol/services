//! Module defining slippage computation for AMM liquidiy.

use crate::{settlement::external_prices::ExternalPrices, solver::Auction};
use anyhow::{Context as _, Result};
use ethcontract::{H160, U256};
use num::{BigInt, BigRational, Integer as _, ToPrimitive as _};
use once_cell::sync::OnceCell;
use std::{borrow::Cow, cmp};

/// Constant maximum slippage of 10 BPS (0.1%) to use for on-chain liquidity.
pub const DEFAULT_MAX_SLIPPAGE_BPS: u32 = 10;

/// Basis points in 100%.
const BPS_BASE: u32 = 10000;

/// A per-auction context for computing slippage.
pub struct SlippageContext<'a> {
    prices: &'a ExternalPrices,
    calculator: &'a SlippageCalculator,
}

impl<'a> SlippageContext<'a> {
    /// Creates a new slippage context.
    pub fn new(prices: &'a ExternalPrices, calculator: &'a SlippageCalculator) -> Self {
        Self { prices, calculator }
    }

    /// Create a new solving context for the specified auction and slippage
    /// calculator.
    pub fn for_auction(auction: &'a Auction, calculator: &'a SlippageCalculator) -> Self {
        Self::new(&auction.external_prices, calculator)
    }

    /// Computes the AMM execution maximum input amount.
    pub fn execution_input_max(&self, input: (H160, U256)) -> Result<(H160, U256)> {
        let (token, amount) = input;
        let slippage = self.calculator.compute(self.prices, token, amount)?;
        Ok((token, slippage.add_to_amount(amount)))
    }

    /// Applies slippage to an input amount.
    pub fn apply_to_amount_in(&self, token: H160, amount: U256) -> Result<U256> {
        Ok(self
            .calculator
            .compute(self.prices, token, amount)?
            .add_to_amount(amount))
    }

    /// Applies slippage to an output amount.
    pub fn apply_to_amount_out(&self, token: H160, amount: U256) -> Result<U256> {
        Ok(self
            .calculator
            .compute(self.prices, token, amount)?
            .sub_from_amount(amount))
    }
}

impl Default for SlippageContext<'static> {
    fn default() -> Self {
        static CONTEXT: OnceCell<(ExternalPrices, SlippageCalculator)> = OnceCell::new();
        let (prices, calculator) = CONTEXT.get_or_init(Default::default);
        Self { prices, calculator }
    }
}

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
        let (relative, absolute) = self.compute_inner(
            external_prices.price(&token),
            number_conversions::u256_to_big_int(&amount),
        )?;
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

    fn compute_inner(
        &self,
        price: Option<&BigRational>,
        amount: BigInt,
    ) -> Result<(Cow<BigRational>, BigInt)> {
        let relative = if let Some(max_absolute_native_token) = self.absolute.clone() {
            let price = price.context("missing token price")?;
            let max_absolute_slippage =
                BigRational::new(max_absolute_native_token, 1.into()) / price;

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
            // Perform a ceil division so that we round up with slippage amount
            ratio.numer().div_ceil(ratio.denom())
        };

        Ok((relative, absolute))
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
    pub fn sub_from_amount(&self, amount: U256) -> U256 {
        amount.saturating_sub(self.absolute)
    }

    /// Increase the specified amount by the constant slippage.
    pub fn add_to_amount(&self, amount: U256) -> U256 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement::external_prices::externalprices;
    use shared::conversions::U256Ext as _;
    use testlib::tokens::{GNO, USDC, WETH};

    #[test]
    fn limits_max_slippage() {
        let calculator = SlippageCalculator::from_bps(10, Some(U256::exp10(17)));
        let prices = externalprices! {
            native_token: WETH,
            GNO => U256::exp10(9).to_big_rational(),
            USDC => BigRational::new(2.into(), 1000.into()),
        };

        for (token, amount, expected_slippage) in [
            (GNO, U256::exp10(12), 1),
            (USDC, U256::exp10(23), 5),
            (GNO, U256::exp10(8), 10),
            (GNO, U256::exp10(17), 0),
        ] {
            let slippage = calculator.compute(&prices, token, amount).unwrap();
            assert_eq!(slippage.as_bps(), expected_slippage);
        }
    }

    #[test]
    fn errors_on_missing_token_price() {
        let calculator = SlippageCalculator::from_bps(10, Some(1_000.into()));
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
        let slippage = SlippageContext::default();
        for (amount, expected) in [
            (0.into(), 0.into()),
            (100.into(), 99.into()),
            (1000.into(), 999.into()),
            (10000.into(), 9990.into()),
            (10001.into(), 9990.into()),
            (
                U256::MAX,
                U256::from_dec_str(
                    "115676297148078879228147414023679219945\
                     416714680974923475418126423905216510295",
                )
                .unwrap(),
            ),
        ] {
            let amount_with_slippage = slippage.apply_to_amount_out(WETH, amount).unwrap();
            assert_eq!(amount_with_slippage, expected);
        }
    }

    #[test]
    fn test_in_amount_with_slippage() {
        let slippage = SlippageContext::default();
        for (amount, expected) in [
            (0.into(), 0.into()),
            (100.into(), 101.into()),
            (1000.into(), 1001.into()),
            (10000.into(), 10010.into()),
            (10001.into(), 10012.into()),
            (U256::MAX, U256::MAX),
        ] {
            let amount_with_slippage = slippage.apply_to_amount_in(WETH, amount).unwrap();
            assert_eq!(amount_with_slippage, expected);
        }
    }
}
