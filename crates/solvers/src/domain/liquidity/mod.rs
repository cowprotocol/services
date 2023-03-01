//! Modelling on-chain liquidity.

pub mod concentrated;
pub mod constant_product;
pub mod limit_order;
pub mod stable;
pub mod weighted_product;

use {
    crate::domain::eth,
    ethereum_types::{H160, U256},
    std::cmp::Ordering,
};

/// A source of liquidity which can be used by the solver.
#[derive(Clone, Debug)]
pub struct Liquidity {
    pub id: Id,
    pub address: H160,
    /// Estimation of gas needed to use this liquidity on-chain.
    pub gas: eth::Gas,
    pub state: State,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Id(pub String);

/// The liquidity state, specific to the type of liquidity.
#[derive(Clone, Debug)]
pub enum State {
    ConstantProduct(constant_product::Pool),
    WeightedProduct(weighted_product::Pool),
    Stable(stable::Pool),
    Concentrated(concentrated::Pool),
    LimitOrder(limit_order::LimitOrder),
}

/// An ordered token pair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TokenPair(eth::TokenAddress, eth::TokenAddress);

impl TokenPair {
    /// Returns a token pair for the given tokens, or `None` if `a` and `b` are
    /// equal.
    pub fn new(a: eth::TokenAddress, b: eth::TokenAddress) -> Option<Self> {
        match a.cmp(&b) {
            Ordering::Less => Some(Self(a, b)),
            Ordering::Equal => None,
            Ordering::Greater => Some(Self(b, a)),
        }
    }

    /// Returns the wrapped token pair as a tuple.
    pub fn get(&self) -> (eth::TokenAddress, eth::TokenAddress) {
        (self.0, self.1)
    }
}

/// A scaling factor used for normalizing token amounts.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScalingFactor(U256);

impl ScalingFactor {
    /// Creates a new scaling factor. Returns `None` if the value is not a power
    /// of 10.
    pub fn new(value: U256) -> Option<Self> {
        if !Self::is_power_of_10(value) {
            return None;
        }
        Some(Self(value))
    }

    /// Returns the underlying scaling factor value.
    pub fn get(&self) -> U256 {
        self.0
    }

    /// Returns the exponent of a scaling factor.
    pub fn exponent(&self) -> u8 {
        let mut factor = self.0;
        let mut exponent = 0_u8;
        while factor > U256::one() {
            factor /= 10;
            exponent += 1;
        }
        exponent
    }

    fn is_power_of_10(mut value: U256) -> bool {
        while value > U256::one() {
            let (quotient, remainder) = value.div_mod(10.into());
            if !remainder.is_zero() {
                return false;
            }
            value = quotient;
        }
        value == U256::one()
    }
}

impl Default for ScalingFactor {
    fn default() -> Self {
        Self(U256::one())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_factor_requires_power_of_10() {
        for result in [
            ScalingFactor::new(0.into()),
            ScalingFactor::new(9.into()),
            ScalingFactor::new(11.into()),
            ScalingFactor::new(90.into()),
            ScalingFactor::new(99.into()),
            ScalingFactor::new(101.into()),
            ScalingFactor::new(110.into()),
            ScalingFactor::new(100010000.into()),
        ] {
            assert!(result.is_none());
        }
    }

    #[test]
    fn scaling_factor_computes_exponent() {
        for i in 0..18 {
            let factor = ScalingFactor::new(U256::from(10).pow(i.into())).unwrap();
            assert_eq!(factor.exponent(), i);
        }
    }
}
