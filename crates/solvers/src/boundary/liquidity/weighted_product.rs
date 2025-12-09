pub use shared::sources::balancer_v2::pool_fetching::WeightedPool as Pool;
use {
    crate::domain::{eth, liquidity},
    alloy::primitives::Address,
    ethereum_types::{H256, U256},
    ethrpc::alloy::conversions::IntoLegacy,
    shared::sources::balancer_v2::{
        pool_fetching::{CommonPoolState, TokenState, WeightedPoolVersion, WeightedTokenState},
        swap::fixed_point::Bfp,
    },
};

/// Converts a domain pool into a [`shared`] Balancer V2 weighted pool. Returns
/// `None` if the domain pool cannot be represented as a boundary pool.
pub fn to_boundary_pool(
    address: Address,
    pool: &liquidity::weighted_product::Pool,
) -> Option<Pool> {
    // NOTE: this is only used for encoding and not for solving, so it's OK to
    // use this an approximate value for now. In fact, Balancer V2 pool IDs
    // are `pool address || pool kind || pool index`, so this approximation is
    // pretty good.
    let id = {
        let mut buf = [0_u8; 32];
        buf[..20].copy_from_slice(address.as_slice());
        H256(buf)
    };

    let swap_fee = to_fixed_point(&pool.fee)?;
    let reserves = pool
        .reserves
        .iter()
        .map(|reserve| {
            Some((
                reserve.asset.token.0,
                WeightedTokenState {
                    common: TokenState {
                        balance: reserve.asset.amount.into_legacy(),
                        scaling_factor: to_fixed_point(&reserve.scale.get())?,
                    },
                    weight: to_fixed_point(&reserve.weight)?,
                },
            ))
        })
        .collect::<Option<_>>()?;

    Some(Pool {
        common: CommonPoolState {
            id,
            address,
            swap_fee,
            paused: false,
        },
        reserves,
        version: match pool.version {
            liquidity::weighted_product::Version::V0 => WeightedPoolVersion::V0,
            liquidity::weighted_product::Version::V3Plus => WeightedPoolVersion::V3Plus,
        },
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
