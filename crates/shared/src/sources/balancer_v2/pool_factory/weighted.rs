//! Module implementing weighted pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::sources::balancer_v2::{graph_api::PoolData, swap::fixed_point::Bfp};
use anyhow::Result;
use contracts::{BalancerV2WeightedPool, BalancerV2WeightedPoolFactory};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
    pub weights: Vec<Bfp>,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: PoolData, block_created: u64) -> Result<Self> {
        todo!()
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2WeightedPoolFactory {
    type PoolInfo = PoolInfo;

    async fn pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        let pool_contract = BalancerV2WeightedPool::at(self.web3(), pool.address);
        let weights = pool_contract
            .methods()
            .get_normalized_weights()
            .call()
            .await?
            .into_iter()
            .map(Bfp::from_wei)
            .collect();

        Ok(PoolInfo {
            common: pool,
            weights,
        })
    }
}
