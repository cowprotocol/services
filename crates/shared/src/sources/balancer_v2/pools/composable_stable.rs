//! Module implementing composable stable pool specific indexing logic.

use {
    super::{common, FactoryIndexing, PoolIndexing},
    crate::{
        ethrpc::Web3CallBatch,
        sources::balancer_v2::{
            graph_api::{PoolData, PoolType},
            swap::fixed_point::Bfp,
        },
    },
    anyhow::Result,
    contracts::{BalancerV2ComposableStablePool, BalancerV2ComposableStablePoolFactory},
    ethcontract::BlockId,
    futures::{future::BoxFuture, FutureExt as _},
};

pub use super::stable::{AmplificationParameter, PoolState};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::ComposableStable, pool, block_created)?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2ComposableStablePoolFactory {
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
        let pool_contract = BalancerV2ComposableStablePool::at(
            &self.raw_instance().web3(),
            pool_info.common.address,
        );

        let scaling_factors = pool_contract
            .get_scaling_factors()
            .block(block)
            .batch_call(batch);
        let amplification_parameter = pool_contract
            .get_amplification_parameter()
            .block(block)
            .batch_call(batch);

        async move {
            let common = common_pool_state.await;
            let scaling_factors = scaling_factors.await?;
            let amplification_parameter = {
                let (factor, _, precision) = amplification_parameter.await?;
                AmplificationParameter::new(factor, precision)?
            };

            Ok(Some(PoolState {
                tokens: common
                    .tokens
                    .into_iter()
                    .zip(scaling_factors)
                    .map(|((address, token), scaling_factor)| {
                        (
                            address,
                            common::TokenState {
                                scaling_factor: Bfp::from_wei(scaling_factor),
                                ..token
                            },
                        )
                    })
                    .collect(),
                swap_fee: common.swap_fee,
                amplification_parameter,
            }))
        }
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sources::balancer_v2::graph_api::Token,
        contracts::dummy_contract,
        ethcontract::{H160, H256},
        ethcontract_mock::Mock,
        futures::future,
        maplit::btreemap,
    };

    #[tokio::test]
    async fn fetch_pool_state() {
        let tokens = btreemap! {
            H160([1; 20]) => common::TokenState {
                    balance: bfp!("1000.0").as_uint256(),
                    scaling_factor: Bfp::exp10(0),
            },
            H160([2; 20]) => common::TokenState {
                    balance: bfp!("10.0").as_uint256(),
                    scaling_factor: bfp!("1.137117595629065656"),
            },
            H160([3; 20]) => common::TokenState {
                    balance: 15_000_000.into(),
                    scaling_factor: Bfp::exp10(12),
            },
        };
        let swap_fee = bfp!("0.00015");
        let amplification_parameter =
            AmplificationParameter::new(200.into(), 10000.into()).unwrap();

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2ComposableStablePool::raw_contract().abi.clone());
        pool.expect_call(
            BalancerV2ComposableStablePool::signatures().get_amplification_parameter(),
        )
        .returns((
            amplification_parameter.factor(),
            false,
            amplification_parameter.precision(),
        ));
        pool.expect_call(BalancerV2ComposableStablePool::signatures().get_scaling_factors())
            .returns(
                tokens
                    .values()
                    .map(|token| token.scaling_factor.as_uint256())
                    .collect(),
            );

        let factory = dummy_contract!(BalancerV2ComposableStablePoolFactory, H160::default());
        let pool_info = PoolInfo {
            common: common::PoolInfo {
                id: H256([0x90; 32]),
                address: pool.address(),
                tokens: tokens.keys().copied().collect(),
                scaling_factors: tokens.values().map(|token| token.scaling_factor).collect(),
                block_created: 1337,
            },
        };
        let common_pool_state = common::PoolState {
            paused: false,
            swap_fee,
            tokens: tokens.clone(),
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

        assert_eq!(
            pool_state,
            Some(PoolState {
                tokens,
                swap_fee,
                amplification_parameter,
            })
        );
    }

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Stable,
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

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }
}
