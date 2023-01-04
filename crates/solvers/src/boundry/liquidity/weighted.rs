use crate::domain::{eth, liquidity};
use ethereum_types::{H160, H256};
pub use shared::sources::balancer_v2::pool_fetching::WeightedPool;
use shared::sources::balancer_v2::{pool_fetching::CommonPoolState, swap::fixed_point::Bfp};

/// Converts a domain pool into a [`shared`] Uniswap V2 pool.
pub fn to_boundry_pool(address: H160, state: &liquidity::weighted::Pool) -> WeightedPool {
    // TODO: this is only used for encoding and not for solving, so it OK to
    // leave this an approximate value for now. In fact, Balancer V2 pool IDs
    // are `pool address || pool kind || pool index`, so this approximation is
    // pretty good.
    let id = {
        let mut buf = [0_u8; 32];
        buf[..20].copy_from_slice(address.as_bytes());
        H256(buf)
    };

    WeightedPool {
        common: CommonPoolState {
            id,
            address,
            swap_fee: to_fixed_point(&state.fee),
            paused: false,
        },
        reserves: todo!(),
    }
}

/// Converts a rational to a Balancer fixed point number.
fn to_fixed_point(ratio: &eth::Rational) -> Bfp {
    todo!()
}
