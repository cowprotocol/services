use {
    crate::domain::{eth, liquidity},
    std::collections::BTreeMap,
};

/// A Uniswap V3 concentrated liquidity pool.
///
/// [^1]: <https://uniswap.org/whitepaper-v3.pdf>
#[derive(Clone, Debug)]
pub struct Pool {
    pub router: eth::ContractAddress,
    pub address: eth::ContractAddress,
    pub tokens: liquidity::TokenPair,
    pub sqrt_price: SqrtPrice,
    pub liquidity: Liquidity,
    pub tick: Tick,
    pub liquidity_net: BTreeMap<Tick, LiquidityNet>,
    pub fee: Fee,
}

/// A compressed representation of the current exchange rate between the tokens
/// belonging to a pool.
///
/// Specifically, this is the representation used in the Uniswap V3 contracts
/// that are needed for amount input and output computation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SqrtPrice(pub eth::U256);

/// An amount of concentrated liquidity within a pool.
///
/// The exact amount in tokens that this liquidity represents is dependant on
/// the current state of the pool.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Liquidity(pub u128);

/// An index to a tick within a concentrated liquidity pool.
///
/// A tick represents a +/- 0.01% partition of the price space where liquidity
/// positions may exist. For more information, consult the
/// [Uniswap V3 documentation](https://docs.uniswap.org/concepts/protocol/concentrated-liquidity#ticks).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Tick(pub i32);

/// The amount of liquidity added (or, if negative, removed) when the tick is
/// crossed going left to right.
#[derive(Debug, Copy, Clone)]
pub struct LiquidityNet(pub i128);

#[derive(Clone, Debug)]
pub struct Fee(pub num::rational::Ratio<u32>);
