//! Module defining slippage computation for AMM liquidity.

use {
    super::AmmOrderExecution,
    anyhow::{Context as _, Result},
    ethcontract::U256,
    num::{BigInt, BigRational, CheckedDiv, Integer as _, ToPrimitive as _},
    once_cell::sync::OnceCell,
    shared::{external_prices::ExternalPrices, http_solver::model::TokenAmount},
    std::{borrow::Cow, cmp},
};

/// Constant maximum slippage of 10 BPS (0.1%) to use for on-chain liquidity.
const DEFAULT_MAX_SLIPPAGE_BPS: u32 = 10;

/// Basis points in 100%.
const BPS_BASE: u32 = 10000;

/// A per-auction context for computing slippage.
pub struct SlippageContext<'a> {
    prices: &'a ExternalPrices,
    calculator: &'a SlippageCalculator,
}

impl<'a> SlippageContext<'a> {
    /// Returns the external prices used for the slippage context.
    pub fn prices(&self) -> &ExternalPrices {
        self.prices
    }

    /// Applies slippage to the specified AMM execution.
    pub fn apply_to_amm_execution(
        &self,
        mut execution: AmmOrderExecution,
    ) -> Result<AmmOrderExecution> {
        let relative_ratio = |token_amount: &TokenAmount| -> Result<Cow<BigRational>> {
            let (relative, _) = self.calculator.compute(
                self.prices.price(&token_amount.token),
                number::conversions::u256_to_big_int(&token_amount.amount),
            )?;
            Ok(relative)
        };

        // It is possible for AMMs to use tokens that don't have external
        // prices. In order to handle these cases, we do in order:
        // 1. Compute the capped slippage using the sell token amount
        // 2. If no sell token price is available, compute the capped slippage using the
        //    buy token amount
        // 3. Fall back to using the default relative slippage without capping
        let relative = if let Ok(relative) = relative_ratio(&execution.input_max) {
            tracing::debug!(
                input_token = ?execution.input_max.token,
                "using AMM input token for capped surplus",
            );
            relative
        } else if let Ok(relative) = relative_ratio(&execution.output) {
            tracing::debug!(
                output_token = ?execution.output.token,
                "using AMM output token for capped surplus",
            );
            relative
        } else {
            tracing::warn!(
                input_token = ?execution.input_max.token,
                output_token = ?execution.output.token,
                "unable to compute capped slippage; falling back to relative slippage",
            );
            Cow::Borrowed(&self.calculator.relative)
        };

        let absolute = absolute_slippage_amount(
            &relative,
            &number::conversions::u256_to_big_int(&execution.input_max.amount),
        );
        let slippage = SlippageAmount::from_num(&relative, &absolute)?;

        if *relative < self.calculator.relative {
            tracing::debug!(
                input_token = ?execution.input_max.token,
                input_amount = ?execution.input_max.token,
                relative = ?slippage.relative,
                absolute = %slippage.absolute,
                "capping AMM slippage to respect maximum absolute amount",
            );
        }

        execution.input_max.amount = slippage.add_to_amount(execution.input_max.amount);
        Ok(execution)
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
    pub relative: BigRational,
    /// The maximum absolute slippage in native tokens.
    pub absolute: Option<BigInt>,
}

impl SlippageCalculator {
    pub fn from_bps(relative_bps: u32, absolute: Option<U256>) -> Self {
        Self {
            relative: BigRational::new(relative_bps.into(), BPS_BASE.into()),
            absolute: absolute.map(|value| number::conversions::u256_to_big_int(&value)),
        }
    }

    pub fn context<'a>(&'a self, prices: &'a ExternalPrices) -> SlippageContext<'a> {
        SlippageContext {
            prices,
            calculator: self,
        }
    }

    /// Computes the capped slippage amount for the specified token price and
    /// amount.
    pub fn compute(
        &self,
        price: Option<&BigRational>,
        amount: BigInt,
    ) -> Result<(Cow<BigRational>, BigInt)> {
        let relative = if let Some(max_absolute_native_token) = self.absolute.clone() {
            let price = price.context("missing token price")?;
            let max_absolute_slippage = BigRational::new(max_absolute_native_token, 1.into())
                .checked_div(price)
                .context("price is zero")?;

            let amount = BigRational::new(amount.clone(), 1.into());

            let max_relative_slippage_respecting_absolute_limit = max_absolute_slippage
                .checked_div(&amount)
                .context("amount is zero")?;

            cmp::min(
                Cow::Owned(max_relative_slippage_respecting_absolute_limit),
                Cow::Borrowed(&self.relative),
            )
        } else {
            Cow::Borrowed(&self.relative)
        };
        let absolute = absolute_slippage_amount(&relative, &amount);

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
        let absolute = number::conversions::big_int_to_u256(absolute)?;

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
}

fn absolute_slippage_amount(relative: &BigRational, amount: &BigInt) -> BigInt {
    let ratio = relative * amount;
    // Perform a ceil division so that we round up with slippage amount
    ratio.numer().div_ceil(ratio.denom())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        shared::externalprices,
        testlib::tokens::{GNO, USDC, WETH},
    };

    #[test]
    fn amm_execution_slippage() {
        let calculator = SlippageCalculator::from_bps(100, Some(U256::exp10(18)));
        let prices = externalprices! { native_token: WETH };

        let slippage = calculator.context(&prices);
        let cases = [
            (
                AmmOrderExecution {
                    input_max: TokenAmount::new(WETH, 1_000_000_000_000_000_000_u128),
                    output: TokenAmount::new(GNO, 10_000_000_000_000_000_000_u128),
                    internalizable: false,
                },
                1_010_000_000_000_000_000_u128.into(),
            ),
            (
                AmmOrderExecution {
                    input_max: TokenAmount::new(GNO, 10_000_000_000_000_000_000_000_u128),
                    output: TokenAmount::new(WETH, 1_000_000_000_000_000_000_000_u128),
                    internalizable: false,
                },
                10_010_000_000_000_000_000_000_u128.into(),
            ),
            (
                AmmOrderExecution {
                    input_max: TokenAmount::new(USDC, 200_000_000_u128),
                    output: TokenAmount::new(GNO, 2_000_000_000_000_000_000_u128),
                    internalizable: false,
                },
                202_000_000_u128.into(),
            ),
        ];

        for (execution, expected) in cases {
            let execution = slippage.apply_to_amm_execution(execution).unwrap();
            assert_eq!(execution.input_max.amount, expected);
        }
    }
}
