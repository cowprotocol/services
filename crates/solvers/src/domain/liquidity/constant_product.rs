//! Constant product pool.

use {crate::domain::eth, std::cmp::Ordering};

/// Uniswap-v2 like pool state.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub fee: eth::Rational,
}

/// Constant product pool reserves.
#[derive(Clone, Debug)]
pub struct Reserves(eth::Asset, eth::Asset);

impl Reserves {
    /// Creates a new constant product pool reserves with the specified assets.
    /// Returns `None` if the assets are denominated in the same tokens.
    pub fn new(a: eth::Asset, b: eth::Asset) -> Option<Self> {
        match a.token.cmp(&b.token) {
            Ordering::Less => Some(Self(a, b)),
            Ordering::Equal => None,
            Ordering::Greater => Some(Self(b, a)),
        }
    }

    /// Get the reserve assets.
    pub fn get(&self) -> (eth::Asset, eth::Asset) {
        (self.0, self.1)
    }
}
