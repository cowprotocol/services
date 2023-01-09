//! Constant product pool.

use crate::domain::eth;
use std::cmp::Ordering;

/// Uniswap-v2 like pool state.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub fee: eth::Rational,
}

/// Constant product reserves.
#[derive(Clone, Debug)]
pub struct Reserves(eth::Asset, eth::Asset);

impl Reserves {
    pub fn new(a: eth::Asset, b: eth::Asset) -> Option<Self> {
        match a.token.cmp(&b.token) {
            Ordering::Less => Some(Self(a, b)),
            Ordering::Equal => None,
            Ordering::Greater => Some(Self(b, a)),
        }
    }

    pub fn get(&self) -> (eth::Asset, eth::Asset) {
        (self.0, self.1)
    }
}
