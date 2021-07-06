//! TODO(nlordell): Migrate from `shared::sources::balancer`.

/// Supported Balancer V2 pools.
pub enum BalancerV2Pool {
    WeightedPool(BalancerV2WeightedPool),
}

pub struct BalancerV2WeightedPool;
