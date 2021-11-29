//! Module implementing two-token weighted pool specific indexing logic.

pub use super::weighted::PoolInfo;
use super::{common, FactoryIndexing};
use anyhow::Result;
use contracts::{BalancerV2WeightedPool2TokensFactory, BalancerV2WeightedPoolFactory};

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2WeightedPool2TokensFactory {
    type PoolInfo = PoolInfo;

    async fn pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        as_weighted_factory(self).pool_info(pool).await
    }
}

fn as_weighted_factory(
    factory: &BalancerV2WeightedPool2TokensFactory,
) -> BalancerV2WeightedPoolFactory {
    BalancerV2WeightedPoolFactory::at(factory.web3(), factory.address())
}
