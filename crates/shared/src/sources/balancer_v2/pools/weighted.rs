//! Module implementing weighted pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing, PoolKind};
use crate::{
    sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    Web3CallBatch,
};
use anyhow::{anyhow, Result};
use contracts::{BalancerV2WeightedPool, BalancerV2WeightedPoolFactory};
use ethcontract::{BlockId, H160};
use futures::{future::BoxFuture, FutureExt as _};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
    pub weights: Vec<Bfp>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub tokens: BTreeMap<H160, TokenState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenState {
    pub common: common::TokenState,
    pub weight: Bfp,
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

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        _: &mut Web3CallBatch,
        _: BlockId,
    ) -> BoxFuture<'static, Result<PoolKind>> {
        let pool_info = pool_info.clone();
        async move {
            let tokens = common_pool_state
                .await
                .tokens
                .into_iter()
                .zip(&pool_info.weights)
                .map(|((address, common), &weight)| (address, TokenState { common, weight }))
                .collect();

            Ok(PoolKind::Weighted(PoolState { tokens }))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::balancer_v2::graph_api::Token;
    use ethcontract::{H160, H256};
    use ethcontract_mock::Mock;
    use futures::future;
    use maplit::btreemap;

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

    #[tokio::test]
    async fn fetch_pool_state() {
        let tokens = btreemap! {
            H160([1; 20]) => common::TokenState {
                balance: bfp!("1000.0").as_uint256(),
                scaling_exponent: 0,
            },
            H160([2; 20]) => common::TokenState {
                balance: 10_000_000.into(),
                scaling_exponent: 12,
            },
        };
        let weights = [bfp!("0.8"), bfp!("0.2")];

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let factory = dummy_contract!(BalancerV2WeightedPoolFactory, H160::default());
        let pool_info = PoolInfo {
            common: common::PoolInfo {
                id: H256([0x90; 32]),
                address: H160([0x90; 20]),
                tokens: tokens.keys().copied().collect(),
                scaling_exponents: tokens
                    .values()
                    .map(|token| token.scaling_exponent)
                    .collect(),
                block_created: 1337,
            },
            weights: weights.to_vec(),
        };
        let common_pool_state = common::PoolState {
            paused: false,
            swap_fee: bfp!("0.003"),
            tokens,
        };

        let pool_state = {
            let mut batch = Web3CallBatch::new(web3.transport().clone());
            let block = web3.eth().block_number().await.unwrap();

            let pool_state = factory.fetch_pool_state(
                &pool_info,
                future::ready(common_pool_state.clone()).boxed(),
                &mut batch,
                block.into(),
            );

            batch.execute_all(100).await;
            pool_state.await.unwrap()
        };

        let weighted_tokens = common_pool_state
            .tokens
            .into_iter()
            .zip(weights)
            .map(|((address, common), weight)| (address, TokenState { common, weight }))
            .collect();
        assert!(matches!(
            pool_state,
            PoolKind::Weighted(pool) if pool == PoolState {
                tokens: weighted_tokens,
            }
        ));
    }
}
