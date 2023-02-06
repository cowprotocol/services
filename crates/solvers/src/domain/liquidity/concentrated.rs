use {
    crate::domain::{eth, liquidity},
    ethereum_types::U256,
    std::collections::BTreeMap,
};

/// State for a UniswapV3-like concentrated liquidity pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub tokens: liquidity::TokenPair,
    pub sqrt_price: SqrtPrice,
    pub liquidity: Amount,
    pub tick: Tick,
    pub liquidity_net: BTreeMap<Tick, Amount>,
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
pub struct Amount(pub U256);

/// An index to a tick within a concentrated liquidity pool.
///
/// A tick represents a +/- 0.01% partition of the price space where liquidity
/// positions may exist. For more information, consult the
/// [Uniswap V3 documentation](https://docs.uniswap.org/concepts/protocol/concentrated-liquidity#ticks).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tick(pub i32);
