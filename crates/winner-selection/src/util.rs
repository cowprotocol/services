//! Utility functions and extensions for winner selection.

use alloy::primitives::U256;

/// Extension trait for U256 to add utility methods.
pub trait U256Ext: Sized {
    /// Ceiling division: (self + other - 1) / other
    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;

    /// Multiply U256 by f64 factor with high precision.
    fn checked_mul_f64(&self, factor: f64) -> Option<Self>;
}

impl U256Ext for U256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(Self::from(1u64))?)?
            .checked_div(*other)
    }

    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        // `factor` is first multiplied by the conversion factor to convert
        // it to integer, to avoid rounding to 0. Then, the result is divided
        // by the conversion factor to convert it back to the original scale.
        //
        // The higher the conversion factor (10^18) the precision is higher. E.g.
        // 0.123456789123456789 will be converted to 123456789123456789.
        const CONVERSION_FACTOR: f64 = 1_000_000_000_000_000_000.;
        let multiplied = self.checked_mul(Self::from(factor * CONVERSION_FACTOR))?
            / Self::from(CONVERSION_FACTOR);
        Some(multiplied)
    }
}
