use crate::domain::{eth, liquidity};
use ethereum_types::{H160, H256, U256};
use shared::sources::balancer_v2::{
    pool_fetching::{CommonPoolState, TokenState, WeightedTokenState},
    swap::fixed_point::Bfp,
};

pub use shared::sources::balancer_v2::pool_fetching::WeightedPool as Pool;

/// Converts a domain pool into a [`shared`] Uniswap V2 pool.
pub fn to_boundary_pool(address: H160, state: &liquidity::weighted::Pool) -> Pool {
    // TODO: this is only used for encoding and not for solving, so it OK to
    // use this an approximate value for now. In fact, Balancer V2 pool IDs
    // are `pool address || pool kind || pool index`, so this approximation is
    // pretty good.
    let id = {
        let mut buf = [0_u8; 32];
        buf[..20].copy_from_slice(address.as_bytes());
        H256(buf)
    };

    Pool {
        common: CommonPoolState {
            id,
            address,
            swap_fee: to_fixed_point(&state.fee),
            paused: false,
        },
        reserves: state
            .reserves
            .iter()
            .map(|reserve| {
                (
                    reserve.asset.token.0,
                    WeightedTokenState {
                        common: TokenState {
                            balance: reserve.asset.amount,
                            scaling_exponent: to_scaling_exponent(&reserve.scale),
                        },
                        weight: to_fixed_point(&reserve.weight),
                    },
                )
            })
            .collect(),
    }
}

/// Converts a rational to a Balancer fixed point number.
fn to_fixed_point(ratio: &eth::Rational) -> Bfp {
    // Balancer "fixed point numbers" are in a weird decimal FP format (instead
    // of a base 2 FP format you typically see). Just convert our ratio into
    // this format.
    // TODO handle overflow.
    Bfp::from_wei(ratio.numer() * U256::from(1_000_000_000_000_000_000_u128) / ratio.denom())
}

/// Converts a scaling factor to its exponent.
fn to_scaling_exponent(factor: &liquidity::balancer::ScalingFactor) -> u8 {
    let mut factor = factor.get();
    let mut exponent = 0_u8;
    while factor > U256::one() {
        factor /= 10;
        exponent += 1;
    }
    exponent
}
