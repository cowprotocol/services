pub use shared::sources::balancer_v2::pool_fetching::StablePool as Pool;
use {
    crate::domain::{eth, liquidity},
    ethereum_types::{H160, H256, U256},
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    shared::sources::balancer_v2::{
        pool_fetching::{AmplificationParameter, CommonPoolState, TokenState},
        swap::fixed_point::Bfp,
    },
};

/// Converts a domain pool into a [`shared`] Balancer V2 stable pool. Returns
/// `None` if the domain pool cannot be represented as a boundary pool.
pub fn to_boundary_pool(address: H160, pool: &liquidity::stable::Pool) -> Option<Pool> {
    // NOTE: this is only used for encoding and not for solving, so it's OK to
    // use this an approximate value for now. In fact, Balancer V2 pool IDs
    // are `pool address || pool kind || pool index`, so this approximation is
    // pretty good.
    let id = {
        let mut buf = [0_u8; 32];
        buf[..20].copy_from_slice(address.as_bytes());
        H256(buf)
    };

    let swap_fee = to_fixed_point(&pool.fee)?;
    let reserves = pool
        .reserves
        .iter()
        .map(|reserve| {
            Some((
                reserve.asset.token.0,
                TokenState {
                    balance: reserve.asset.amount.into_legacy(),
                    scaling_factor: to_fixed_point(&reserve.scale.get())?,
                },
            ))
        })
        .collect::<Option<_>>()?;
    let amplification_parameter = AmplificationParameter::try_new(
        pool.amplification_parameter.numer().into_legacy(),
        pool.amplification_parameter.denom().into_legacy(),
    )
    .ok()?;

    Some(Pool {
        common: CommonPoolState {
            id,
            address: address.into_alloy(),
            swap_fee,
            paused: false,
        },
        reserves,
        amplification_parameter,
    })
}

/// Converts a rational to a Balancer fixed point number.
fn to_fixed_point(ratio: &eth::Rational) -> Option<Bfp> {
    // Balancer "fixed point numbers" are in a weird decimal FP format (instead
    // of a base 2 FP format you typically see). Just convert our ratio into
    // this format.
    let base = U256::exp10(18);
    let wei = ratio.numer().into_legacy().checked_mul(base)? / ratio.denom().into_legacy();
    Some(Bfp::from_wei(wei))
}
