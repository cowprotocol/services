pub use shared::sources::uniswap_v2::pool_fetching::Pool;
use {crate::domain::liquidity, ethereum_types::H160, model::TokenPair};

/// Converts a domain pool into a [`shared`] Uniswap V2 pool. Returns `None` if
/// the domain pool cannot be represented as a boundary pool.
pub fn to_boundary_pool(address: H160, pool: &liquidity::constant_product::Pool) -> Option<Pool> {
    let reserves = pool.reserves.get();
    let tokens = TokenPair::new(reserves.0.token.0, reserves.1.token.0)
        .expect("tokens are distinct by construction");

    // reserves are ordered by construction.
    let reserves = (reserves.0.amount.as_u128(), reserves.1.amount.as_u128());

    if *pool.fee.numer() > u32::MAX.into() || *pool.fee.denom() > u32::MAX.into() {
        return None;
    }
    let fee = num::rational::Ratio::new(pool.fee.numer().as_u32(), pool.fee.denom().as_u32());

    Some(Pool {
        address,
        tokens,
        reserves,
        fee,
    })
}
