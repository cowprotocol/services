/// Liquidity data tied to a Balancer V2 pool based on "Weighted Math" [^1].
///
/// Balancer V2 supports two kinds of pools that fall in this category:
/// - Weighted Pools [^2]
/// - Liquidity Bootstrapping Pools [^3]
///
/// Both of these pools have an identical representation, and are therefore
/// modelled by the same type.
///
/// [^1]: <https://docs.balancer.fi/concepts/math/weighted-math>
/// [^2]: <https://docs.balancer.fi/products/balancer-pools/weighted-pools>
/// [^3]: <https://docs.balancer.fi/products/balancer-pools/liquidity-bootstrapping-pools-lbps>
#[derive(Clone, Debug)]
pub struct Pool {}
