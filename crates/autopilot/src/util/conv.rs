use crate::domain::eth;

pub trait U256Ext: Sized {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;
    fn checked_mul_f64(&self, factor: f64) -> Option<Self>;
}

impl U256Ext for eth::U256 {
    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }

    fn checked_mul_f64(&self, factor: f64) -> Option<Self> {
        // `factor` is first multiplied by the conversion factor to convert
        // it to integer, to avoid rounding to 0. Then, the result is divided
        // by the conversion factor to convert it back to the original scale.
        //
        // The higher the conversion factor (10^18) the precision is higher. E.g.
        // 0.123456789123456789 will be converted to 123456789123456789.
        // TODO: consider doing the computation with `BigRational` instead but
        // that requires to double check and adjust a few tests due to tiny
        // changes in rounding.
        const CONVERSION_FACTOR: f64 = 1_000_000_000_000_000_000.;
        let multiplied = self.checked_mul(Self::from_f64_lossy(factor * CONVERSION_FACTOR))?
            / Self::from_f64_lossy(CONVERSION_FACTOR);
        Some(multiplied)
    }
}
