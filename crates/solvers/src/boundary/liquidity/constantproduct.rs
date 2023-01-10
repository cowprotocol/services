use crate::domain::liquidity;
use ethereum_types::H160;
use model::TokenPair;

pub use shared::sources::uniswap_v2::pool_fetching::Pool;

/// Converts a domain pool into a [`shared`] Uniswap V2 pool. Returns `None` if
/// the domain pool cannot be represented as a boundary pool.
pub fn to_boundary_pool(address: H160, state: &liquidity::constantproduct::Pool) -> Option<Pool> {
    let reserves = state.reserves.get();
    let tokens = TokenPair::new(reserves.0.token.0, reserves.1.token.0)
        .expect("tokens are distinct by construction");

    // reserves are ordered by construction.
    let reserves = (reserves.0.amount.as_u128(), reserves.1.amount.as_u128());

    if *state.fee.numer() > u32::MAX.into() || *state.fee.denom() > u32::MAX.into() {
        return None;
    }
    let fee = num::rational::Ratio::new(state.fee.numer().as_u32(), state.fee.denom().as_u32());

    Some(Pool {
        address,
        tokens,
        reserves,
        fee,
    })
}
