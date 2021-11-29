//! Module implementing stable pool specific indexing logic.

pub use super::{common::PoolInfo, FactoryIndexing};
use anyhow::Result;
use contracts::BalancerV2StablePoolFactory;

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2StablePoolFactory {
    type PoolInfo = PoolInfo;

    async fn pool_info(&self, pool: PoolInfo) -> Result<Self::PoolInfo> {
        Ok(pool)
    }
}
