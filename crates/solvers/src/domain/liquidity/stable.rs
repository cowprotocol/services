use crate::domain::{eth, liquidity};
use itertools::Itertools as _;

/// The state of a Curve-like stable pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub amplification_parameter: eth::Rational,
    pub fee: eth::Rational,
}

/// A reprensentation of BalancerV2-like weighted pool reserves.
#[derive(Clone, Debug)]
pub struct Reserves(Vec<Reserve>);

impl Reserves {
    /// Returns a new reserve instance for specified reserve entries. Returns
    /// `None` if it encounters duplicate entries for a token.
    pub fn new(mut reserves: Vec<Reserve>) -> Option<Self> {
        // Note that we sort the reserves by their token address. This is
        // because BalancerV2 stable pools store their tokens in sorting order
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

    /// Returns an iterator over the tokens pairs handled by the pool reserves.
    pub fn token_pairs(&self) -> impl Iterator<Item = liquidity::TokenPair> + '_ {
        self.0
            .iter()
            .tuple_combinations()
            .map(|(a, b)| liquidity::TokenPair::new(a.asset.token, b.asset.token).expect("a != b"))
    }
}

/// A weighted pool token reserve.
#[derive(Clone, Debug)]
pub struct Reserve {
    pub asset: eth::Asset,
    pub scale: liquidity::ScalingFactor,
}
