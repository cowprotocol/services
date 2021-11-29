//! Module implementing weighted pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::sources::balancer_v2::{
    graph_api::{PoolData, PoolType},
    swap::fixed_point::Bfp,
};
use anyhow::{anyhow, ensure, Result};
use contracts::BalancerV2WeightedPoolFactory;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
    pub weights: Vec<Bfp>,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        ensure!(
            pool.pool_type == PoolType::Weighted,
            "cannot convert {:?} pool to weighted pool",
            pool.pool_type,
        );

        Ok(PoolInfo {
            common: common::PoolInfo::from_graph_data(pool, block_created)?,
            weights: pool
                .tokens
                .iter()
                .map(|token| {
                    token
                        .weight
                        .ok_or_else(|| anyhow!("missing weights for pool {:?}", pool.id))
                })
                .collect::<Result<_>>()?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2WeightedPoolFactory {
    type PoolInfo = PoolInfo;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::graph_api::Token;
    use ethcontract::{H160, H256};

    #[test]
    fn convert_graph_pool_to_weighted_pool_info() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0xfa; 20]),
            tokens: vec![
                Token {
                    address: H160([0x11; 20]),
                    decimals: 1,
                    weight: Some("1.337".parse().unwrap()),
                },
                Token {
                    address: H160([0x22; 20]),
                    decimals: 2,
                    weight: Some("4.2".parse().unwrap()),
                },
            ],
        };

        assert_eq!(
            PoolInfo::from_graph_data(&pool, 42).unwrap(),
            PoolInfo {
                common: common::PoolInfo {
                    id: H256([2; 32]),
                    address: H160([1; 20]),
                    tokens: vec![H160([0x11; 20]), H160([0x22; 20])],
                    scaling_exponents: vec![17, 16],
                    block_created: 42,
                },
                weights: vec![
                    Bfp::from_wei(1_337_000_000_000_000_000u128.into()),
                    Bfp::from_wei(4_200_000_000_000_000_000u128.into()),
                ],
            },
        );
    }

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0xfa; 20]),
            tokens: vec![
                Token {
                    address: H160([0x11; 20]),
                    decimals: 1,
                    weight: Some("1.337".parse().unwrap()),
                },
                Token {
                    address: H160([0x22; 20]),
                    decimals: 2,
                    weight: Some("4.2".parse().unwrap()),
                },
            ],
        };

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }
}
