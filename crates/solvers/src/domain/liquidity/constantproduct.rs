//! Constant product pool.

use crate::domain::eth;
use std::ops::Index;

/// Uniswap-v2 like pool state.
#[derive(Clone, Debug)]
pub struct Pool {
    pub reserves: Reserves,
    pub fee: eth::Rational,
}

/// Constant product reserves.
#[derive(Clone, Debug)]
pub struct Reserves([eth::Asset; 2]);

/// A token index for pool reserves.
pub enum TokenIndex {
    Zero = 0,
    One = 1,
}

impl Index<TokenIndex> for Reserves {
    type Output = eth::Asset;

    fn index(&self, index: TokenIndex) -> &Self::Output {
        &self.0[index as usize]
    }
}
