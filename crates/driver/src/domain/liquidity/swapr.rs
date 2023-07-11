use crate::domain::{
    eth,
    liquidity::{self, uniswap},
};

/// The famous Uniswap V2 constant product pool with a twist of lemon,
/// specifically Swapr uses custom fees per pool instead of the constant 0.3%.
#[derive(Clone, Debug)]
pub struct Pool {
    pub base: uniswap::v2::Pool,
    pub fee: Fee,
}

/// A swap fee.
///
/// Internally, it is represented in basis points.
#[derive(Clone, Copy, Debug)]
pub struct Fee(u32);

impl Pool {
    /// Encodes a pool swap as an interaction. Returns `None` if the swap
    /// parameters are invalid for the pool, specifically if the input and
    /// output tokens don't correspond to the pool's token pair.
    pub fn swap(
        &self,
        input: &liquidity::MaxInput,
        output: &liquidity::ExactOutput,
        receiver: &eth::Address,
    ) -> Option<eth::Interaction> {
        // Note that swap interactions are identical in Swapr and Uniswap V2
        // pools. The only difference is the input/output computation uses
        // different fees.
        self.base.swap(input, output, receiver)
    }
}

impl Fee {
    /// Creates a new fee from the specified basis points. Returns `None` for
    /// invalid fee values (i.e. outside the range `[0, 1000]`).
    pub fn new(bps: u32) -> Option<Self> {
        if !(0..=1000).contains(&bps) {
            return None;
        }
        Some(Self(bps))
    }
}
