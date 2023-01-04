use crate::domain::eth;
use ethereum_types::U256;
use std::collections::BTreeMap;

/// State for a UniswapV3-like concentrated liquidity pool.
pub struct Pool {
    pub tokens: TokenPair,
    pub sqrt_price: U256,
    pub liquidity: U256,
    pub tick: i32,
    pub liquidity_net: BTreeMap<i32, U256>,
    pub fee: eth::Rational,
}

/// An ordered token pair.
pub struct TokenPair([eth::TokenAddress; 2]);
