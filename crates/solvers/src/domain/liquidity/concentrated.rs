use crate::domain::{eth, liquidity};
use ethereum_types::U256;
use std::collections::BTreeMap;

/// State for a UniswapV3-like concentrated liquidity pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub tokens: liquidity::TokenPair,
    pub sqrt_price: SqrtPrice,
    pub liquidity: LiquidityAmount,
    pub tick: Tick,
    pub liquidity_net: BTreeMap<Tick, LiquidityAmount>,
    pub fee: eth::Rational,
}

/// A compressed representation of the current exchange rate between the tokens
/// belonging to a pool.
///
/// Specifically, this is the representation used in the Uniswap V3 contracts
/// that are needed for amount input and output computation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SqrtPrice(pub U256);

/// An amount of concetrated liquidity within a pool.
///
/// The exact amount in tokens that this liquidity represents is dependant on
/// the current state of the pool.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LiquidityAmount(pub U256);

/// A liquidity tick.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tick(pub i32);
