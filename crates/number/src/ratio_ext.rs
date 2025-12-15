use {
    alloy::primitives::{U256, U512, ruint::UintTryFrom},
    num::rational::Ratio,
};

pub trait RatioExt<N> {
    const ZERO: Ratio<N>;
    const ONE: Ratio<N>;

    /// Multiplies a ratio by a scalar, returning `None` if the result or any
    /// intermediate operation would overflow a `U256`.
    fn scalar_mul(&self, scalar: N) -> Option<N>;

    /// Multiplies a ratio by a scalar, returning `None` only if the result
    /// would overflow a `U256`, but intermediate operations are allowed to
    /// overflow.
    fn full_scalar_mul(&self, scalar: N) -> Option<N>;
}

impl RatioExt<U256> for Ratio<U256> {
    const ONE: Ratio<U256> = Ratio::new_raw(U256::ONE, U256::ONE);
    const ZERO: Ratio<U256> = Ratio::new_raw(U256::ZERO, U256::ONE);

    fn scalar_mul(&self, scalar: U256) -> Option<U256> {
        scalar
            .checked_mul(*self.numer())?
            .checked_div(*self.denom())
    }

    fn full_scalar_mul(&self, scalar: U256) -> Option<U256> {
        U256::uint_try_from(
            scalar
                .widening_mul(*self.numer())
                .checked_div(U512::from(*self.denom()))?,
        )
        .ok()
    }
}
