use crate::domain::liquidity;

/// State for a UniswapV3-like concentrated liquidity pool.
#[derive(Clone, Debug)]
pub struct Pool {
    pub tokens: liquidity::TokenPair,
    pub fee: Fee,
}

/// Amount of fees accrued when using this pool.
/// Uniswap v3 was launched with 3 fee tiers (5, 30, 100 bps) but more could be
/// added by the uniswap DAO.
#[derive(Clone, Debug)]
pub struct Fee(pub u32);
