//! Module implementing stable pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::sources::balancer_v2::graph_api::{PoolData, PoolType};
use anyhow::Result;
use contracts::BalancerV2StablePoolFactory;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::Stable, pool, block_created)?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2StablePoolFactory {
    type PoolInfo = PoolInfo;

    async fn pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        Ok(PoolInfo { common: pool })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::graph_api::Token;
    use ethcontract::{H160, H256};

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0xfa; 20]),
            tokens: vec![
                Token {
                    address: H160([0x11; 20]),
                    decimals: 1,
                    weight: None,
                },
                Token {
                    address: H160([0x22; 20]),
                    decimals: 2,
                    weight: None,
                },
            ],
        };

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }
}
