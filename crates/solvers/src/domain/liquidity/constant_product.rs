//! Constant product pool.

use {
    crate::domain::{eth, liquidity},
    alloy::primitives::U256,
    std::cmp::Ordering,
};

/// Uniswap-v2 like pool state.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub fee: eth::Rational,
}

impl Pool {
    /// Returns the pool's token pair.
    pub fn tokens(&self) -> liquidity::TokenPair {
        liquidity::TokenPair::new(self.reserves.0.token, self.reserves.1.token)
            .expect("pool reserve assets have different tokens")
    }

    /// Returns the constant product pool's `k` value. This is the product of
    /// the pool's token balances.
    pub fn k(&self) -> U256 {
        self.reserves
            .0
            .amount
            .checked_mul(self.reserves.1.amount)
            .expect("product of two u96 cannot overflow a u256")
    }
}

/// Constant product pool reserves.
#[derive(Clone, Debug)]
pub struct Reserves(eth::Asset, eth::Asset);

impl Reserves {
    /// Creates a new constant product pool reserves with the specified assets.
    /// Returns `None` if the assets are denominated in the same token or if the
    /// balances are larger than the maximum allowed values.
    pub fn new(a: eth::Asset, b: eth::Asset) -> Option<Self> {
        // UniswapV2-Like constant product pools are limited to uint112 values
        // for token reserves - so verify this invariant.
        let max = U256::from(2_u128.pow(112) - 1);
        if a.amount > max || b.amount > max {
            return None;
        }

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
