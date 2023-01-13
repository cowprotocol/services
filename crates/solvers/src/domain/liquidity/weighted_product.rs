use crate::domain::{eth, liquidity};
use ethereum_types::U256;
use itertools::Itertools as _;

/// The state of a Balancer-like weighted product pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub fee: eth::Rational,
}

impl Pool {
    /// Returns an iterator over the tokens pairs handled by this pool.
    pub fn token_pairs(&self) -> impl Iterator<Item = liquidity::TokenPair> + '_ {
        self.reserves
            .0
            .iter()
            .tuple_combinations()
            .map(|(a, b)| liquidity::TokenPair::new(a.asset.token, b.asset.token).expect("a != b"))
    }
}

/// A reprensentation of BalancerV2-like weighted pool reserves.
#[derive(Clone, Debug)]
pub struct Reserves(Vec<Reserve>);

impl Reserves {
    /// Returns a new reserve instance for specified reserve entries. Returns
    /// `None` if it encounters duplicate entries for a token.
    pub fn new(mut reserves: Vec<Reserve>) -> Option<Self> {
        // Note that we sort the reserves by their token address. This is
        // because BalancerV2 weighted pools store their tokens in sorting order
        // - meaning that `token0` is the token address with the lowest sort
        // order. This ensures that this iterator returns the token reserves in
        // the correct order.
        reserves.sort_unstable_by_key(|reserve| reserve.asset.token);

        let has_duplicates = reserves
            .iter()
            .tuple_windows()
            .any(|(a, b)| a.asset.token == b.asset.token);
        if has_duplicates {
            return None;
        }

        Some(Self(reserves))
    }

    /// Returns an iterator over the token reserves.
    pub fn iter(&self) -> impl Iterator<Item = Reserve> + '_ {
        self.0.iter().cloned()
    }

    /// Returns the reserve for the specified token.
    pub fn get(&self, token: eth::TokenAddress) -> Option<Reserve> {
        let index = self
            .0
            .binary_search_by_key(&token, |reserve| reserve.asset.token)
            .ok()?;
        Some(self.0[index].clone())
    }
}

/// A weighted pool token reserve.
#[derive(Clone, Debug)]
pub struct Reserve {
    pub asset: eth::Asset,
    pub weight: eth::Rational,
    pub scale: ScalingFactor,
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
