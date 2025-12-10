pub use shared::sources::uniswap_v2::pool_fetching::Pool;
use {crate::domain::liquidity, alloy::primitives::Address, model::TokenPair};

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
        u128::try_from(reserves.0.amount)
            .inspect_err(|_| {
                tracing::debug!(
                    address = %reserves.0.token.0,
                    "asset 0 amount > u128"
                );
            })
            .ok()?,
        u128::try_from(reserves.1.amount)
            .inspect_err(|_| {
                tracing::debug!(
                    address = %reserves.1.token.0,
                    "asset 1 amount > u128"
                );
            })
            .ok()?,
    );

    let fee = num::rational::Ratio::new(
        u32::try_from(pool.fee.numer())
            .inspect_err(
                |_| tracing::debug!(pool = ?pool.reserves, "pool fee numerator > u32::MAX"),
            )
            .ok()?,
        u32::try_from(pool.fee.denom())
            .inspect_err(
                |_| tracing::debug!(pool = ?pool.reserves, "pool fee denominator > u32::MAX"),
            )
            .ok()?,
    );

    Some(Pool {
        address,
        tokens,
        reserves,
        fee,
    })
}
