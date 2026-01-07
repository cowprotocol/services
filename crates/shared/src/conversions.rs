// Re-export U256Ext from number crate
pub use number::u256_ext::U256Ext;
use {
    anyhow::{Result, ensure},
    num::rational::Ratio,
};

// Convenience:

pub trait RatioExt<T> {
    fn new_checked(numerator: T, denominator: T) -> Result<Ratio<T>>;
}

impl<T: num::Integer + Clone> RatioExt<T> for Ratio<T> {
    fn new_checked(numerator: T, denominator: T) -> Result<Ratio<T>> {
        ensure!(
            !denominator.is_zero(),
            "Cannot create Ratio with 0 denominator"
        );
        Ok(Ratio::new(numerator, denominator))
    }
}
