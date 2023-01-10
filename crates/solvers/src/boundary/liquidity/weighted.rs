use crate::domain::{eth, liquidity};
use ethereum_types::{H160, H256, U256};
use shared::sources::balancer_v2::{
    pool_fetching::{CommonPoolState, TokenState, WeightedTokenState},
    swap::fixed_point::Bfp,
};
use std::collections::HashMap;

pub use shared::sources::balancer_v2::pool_fetching::WeightedPool as Pool;

/// Converts a domain pool into a [`shared`] Balancer V2 weighted pool. Returns
/// `None` if the domain pool cannot be represented as a boundary pool.
pub fn to_boundary_pool(address: H160, state: &liquidity::weighted::Pool) -> Option<Pool> {
    // NOTE: this is only used for encoding and not for solving, so it OK to
    // use this an approximate value for now. In fact, Balancer V2 pool IDs
    // are `pool address || pool kind || pool index`, so this approximation is
    // pretty good.
    let id = {
        let mut buf = [0_u8; 32];
        buf[..20].copy_from_slice(address.as_bytes());
        H256(buf)
    };

    let swap_fee = to_fixed_point(&state.fee)?;
    let reserves = state
        .reserves
        .iter()
        .map(|reserve| {
            Some((
                reserve.asset.token.0,
                WeightedTokenState {
                    common: TokenState {
                        balance: reserve.asset.amount,
                        scaling_exponent: reserve.scale.exponent(),
                    },
                    weight: to_fixed_point(&reserve.weight)?,
                },
            ))
        })
        .collect::<Option<HashMap<_, _>>>()?;

    Some(Pool {
        common: CommonPoolState {
            id,
            address,
            swap_fee,
            paused: false,
        },
        reserves,
    })
}

/// Converts a rational to a Balancer fixed point number.
fn to_fixed_point(ratio: &eth::Rational) -> Option<Bfp> {
    // Balancer "fixed point numbers" are in a weird decimal FP format (instead
    // of a base 2 FP format you typically see). Just convert our ratio into
    // this format.
    let base = U256::from(1_000_000_000_000_000_000_u128);
    let wei = ratio.numer().checked_mul(base)? / ratio.denom();
    Some(Bfp::from_wei(wei))
}
