use crate::domain::liquidity;
use ethereum_types::H160;
use model::TokenPair;

pub use shared::sources::uniswap_v2::pool_fetching::Pool;

/// Converts a domain pool into a [`shared`] Uniswap V2 pool.
pub fn to_boundary_pool(address: H160, state: &liquidity::constantproduct::Pool) -> Pool {
    let tokens = TokenPair::new(*state.reserves[0].token, *state.reserves[1].token)
        .expect("tokens are distinct by construction");
    // reserves are ordered by construction.
    let reserves = (
        state.reserves[0].amount.as_u128(),
        state.reserves[1].amount.as_u128(),
    );

    Pool {
        address,
        tokens,
        reserves,
        // TODO: potentially handle overflows...
        fee: num::rational::Ratio::new(state.fee.numer().as_u32(), state.fee.numer().as_u32()),
    }
}
