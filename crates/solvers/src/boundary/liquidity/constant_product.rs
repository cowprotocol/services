pub use shared::sources::uniswap_v2::pool_fetching::Pool;
use {
    crate::domain::liquidity,
    alloy::primitives::{Address, U256},
    model::TokenPair,
};

/// Converts a domain pool into a [`shared`] Uniswap V2 pool. Returns `None` if
/// the domain pool cannot be represented as a boundary pool.
pub fn to_boundary_pool(
    address: Address,
    pool: &liquidity::constant_product::Pool,
) -> Option<Pool> {
    let reserves = pool.reserves.get();
    let tokens = TokenPair::new(reserves.0.token.0, reserves.1.token.0)
        .expect("tokens are distinct by construction");

    // reserves are ordered by construction.
    let reserves = (
        u128::try_from(reserves.0.amount).expect("value should be lower than u128::MAX"),
        u128::try_from(reserves.1.amount).expect("value should be lower than u128::MAX"),
    );

    if *pool.fee.numer() > U256::from(u32::MAX) || *pool.fee.denom() > U256::from(u32::MAX) {
        return None;
    }
    let fee = num::rational::Ratio::new(
        u32::try_from(pool.fee.numer()).expect("previous check should ensure that n <= u32::MAX"),
        u32::try_from(pool.fee.denom()).expect("previous check should ensure that n <= u32::MAX"),
    );

    Some(Pool {
        address,
        tokens,
        reserves,
        fee,
    })
}
