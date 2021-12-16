//! Module implementing weighted pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::sources::balancer_v2::{
    graph_api::{PoolData, PoolType},
    swap::fixed_point::Bfp,
};
use anyhow::{anyhow, Result};
use contracts::{BalancerV2WeightedPool, BalancerV2WeightedPoolFactory};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
    pub weights: Vec<Bfp>,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::Weighted, pool, block_created)?,
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

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        let pool_contract = BalancerV2WeightedPool::at(&self.raw_instance().web3(), pool.address);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::graph_api::Token;
    use ethcontract::{H160, H256};
    use ethcontract_mock::Mock;

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
                    weight: Some(bfp!("1.337")),
                },
                Token {
                    address: H160([0x22; 20]),
                    decimals: 2,
                    weight: Some(bfp!("4.2")),
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
                    weight: Some(bfp!("1.337")),
                },
                Token {
                    address: H160([0x22; 20]),
                    decimals: 2,
                    weight: Some(bfp!("4.2")),
                },
            ],
        };

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[tokio::test]
    async fn fetch_weighted_pool() {
        let weights = [bfp!("0.5"), bfp!("0.25"), bfp!("0.25")];

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2WeightedPool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2WeightedPool::signatures().get_normalized_weights())
            .returns(weights.iter().copied().map(Bfp::as_uint256).collect());

        let factory = BalancerV2WeightedPoolFactory::at(&web3, H160([0xfa; 20]));
        let pool = factory
            .specialize_pool_info(common::PoolInfo {
                id: H256([0x90; 32]),
                tokens: vec![H160([1; 20]), H160([2; 20]), H160([3; 20])],
                address: pool.address(),
                scaling_exponents: vec![0, 0, 0],
                block_created: 42,
            })
            .await
            .unwrap();

        assert_eq!(pool.weights, weights);
    }
}
