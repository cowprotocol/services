//! Module implementing liquidity bootstrapping pool specific indexing logic.

pub use super::weighted::{PoolState, TokenState};
use super::{common, FactoryIndexing, PoolIndexing};
use crate::{
    event_handling::BlockNumberHash,
    sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    Web3CallBatch,
};
use anyhow::Result;
use contracts::{
    BalancerV2LiquidityBootstrappingPool, BalancerV2LiquidityBootstrappingPoolFactory,
};
use ethcontract::BlockId;
use futures::{future::BoxFuture, FutureExt as _};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: BlockNumberHash) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(
                PoolType::LiquidityBootstrapping,
                pool,
                block_created,
            )?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2LiquidityBootstrappingPoolFactory {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        Ok(PoolInfo { common: pool })
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        let pool_contract = BalancerV2LiquidityBootstrappingPool::at(
            &self.raw_instance().web3(),
            pool_info.common.address,
        );

        // Liquidity bootstrapping pools use dynamic weights, meaning that we
        // need to fetch them every time.
        let weights = pool_contract
            .get_normalized_weights()
            .block(block)
            .batch_call(batch);
        let swap_enabled = pool_contract
            .get_swap_enabled()
            .block(block)
            .batch_call(batch);

        async move {
            if !swap_enabled.await? {
                return Ok(None);
            }

            let common = common_pool_state.await;
            let weights = weights.await?;
            let tokens = common
                .tokens
                .into_iter()
                .zip(&weights)
                .map(|((address, common), &weight)| {
                    (
                        address,
                        TokenState {
                            common,
                            weight: Bfp::from_wei(weight),
                        },
                    )
                })
                .collect();
            let swap_fee = common.swap_fee;

            Ok(Some(PoolState { tokens, swap_fee }))
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

    #[tokio::test]
    async fn fetch_pool_state() {
        let tokens = btreemap! {
            H160([1; 20]) => TokenState {
                common: common::TokenState {
                    balance: bfp!("1000.0").as_uint256(),
                    scaling_exponent: 0,
                },
                weight: bfp!("0.5"),
            },
            H160([2; 20]) => TokenState {
                common: common::TokenState {
                    balance: bfp!("10.0").as_uint256(),
                    scaling_exponent: 0,
                },
                weight: bfp!("0.3"),
            },
            H160([3; 20]) => TokenState {
                common: common::TokenState {
                    balance: 15_000_000.into(),
                    scaling_exponent: 12,
                },
                weight: bfp!("0.2"),
            },
        };
        let swap_fee = bfp!("0.00015");

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(
            BalancerV2LiquidityBootstrappingPool::raw_contract()
                .abi
                .clone(),
        );
        pool.expect_call(
            BalancerV2LiquidityBootstrappingPool::signatures().get_normalized_weights(),
        )
        .returns(
            tokens
                .values()
                .map(|token| token.weight.as_uint256())
                .collect(),
        );
        pool.expect_call(BalancerV2LiquidityBootstrappingPool::signatures().get_swap_enabled())
            .returns(true);

        let factory = dummy_contract!(BalancerV2LiquidityBootstrappingPoolFactory, H160::default());
        let pool_info = PoolInfo {
            common: common::PoolInfo {
                id: H256([0x90; 32]),
                address: pool.address(),
                tokens: tokens.keys().copied().collect(),
                scaling_exponents: tokens
                    .values()
                    .map(|token| token.common.scaling_exponent)
                    .collect(),
                block_created: (1337, Some(H256::from_low_u64_be(1337))),
            },
        };
        let common_pool_state = common::PoolState {
            paused: false,
            swap_fee,
            tokens: tokens
                .iter()
                .map(|(address, token)| (*address, token.common.clone()))
                .collect(),
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

        assert_eq!(pool_state, Some(PoolState { tokens, swap_fee }));
    }

    #[tokio::test]
    async fn returns_none_if_swaps_disabled() {
        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(
            BalancerV2LiquidityBootstrappingPool::raw_contract()
                .abi
                .clone(),
        );
        pool.expect_call(
            BalancerV2LiquidityBootstrappingPool::signatures().get_normalized_weights(),
        )
        .returns(vec![bfp!("0.5").as_uint256(), bfp!("0.5").as_uint256()]);
        pool.expect_call(BalancerV2LiquidityBootstrappingPool::signatures().get_swap_enabled())
            .returns(false);

        let factory = dummy_contract!(BalancerV2LiquidityBootstrappingPoolFactory, H160::default());
        let pool_info = PoolInfo {
            common: common::PoolInfo {
                id: H256([0x90; 32]),
                address: pool.address(),
                tokens: vec![H160([1; 20]), H160([1; 20])],
                scaling_exponents: vec![0, 0],
                block_created: (1337, Some(H256::from_low_u64_be(1337))),
            },
        };
        let common_pool_state = common::PoolState {
            paused: false,
            swap_fee: Bfp::zero(),
            tokens: btreemap! {
                H160([1; 20]) => common::TokenState {
                    balance: 0.into(),
                    scaling_exponent: 0,
                },
                H160([1; 20]) => common::TokenState {
                    balance: 0.into(),
                    scaling_exponent: 0,
                },
            },
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

        assert_eq!(pool_state, None);
    }

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: H160([1; 20]),
            factory: H160([0xfa; 20]),
            swap_enabled: true,
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

        assert!(PoolInfo::from_graph_data(&pool, (42, Some(H256::from_low_u64_be(42)))).is_err());
    }
}
