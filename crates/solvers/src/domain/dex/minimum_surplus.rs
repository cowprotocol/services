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
