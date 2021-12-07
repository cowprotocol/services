//! Module implementing two-token weighted pool specific indexing logic.

pub use super::weighted::PoolInfo;
use super::FactoryIndexing;
use contracts::BalancerV2WeightedPool2TokensFactory;

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2WeightedPool2TokensFactory {
    type PoolInfo = PoolInfo;
}
