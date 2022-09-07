//! Module implementing stable pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::{
    conversions::U256Ext as _,
    event_handling::BlockNumberHash,
    sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    Web3CallBatch,
};
use anyhow::{ensure, Result};
use contracts::{BalancerV2StablePool, BalancerV2StablePoolFactory, BalancerV2StablePoolFactoryV2};
use ethcontract::{BlockId, H160, U256};
use futures::{future::BoxFuture, FutureExt as _};
use num::BigRational;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: BlockNumberHash) -> Result<Self> {
        Ok(PoolInfo {
            common: common::PoolInfo::for_type(PoolType::Stable, pool, block_created)?,
        })
    }

    fn common(&self) -> &common::PoolInfo {
        &self.common
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub tokens: BTreeMap<H160, common::TokenState>,
    pub swap_fee: Bfp,
    pub amplification_parameter: AmplificationParameter,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AmplificationParameter {
    factor: U256,
    precision: U256,
}

impl AmplificationParameter {
    pub fn new(factor: U256, precision: U256) -> Result<Self> {
        ensure!(!precision.is_zero(), "Zero precision not allowed");
        Ok(Self { factor, precision })
    }

    /// This is the format used to pass into smart contracts.
    pub fn as_u256(&self) -> U256 {
        self.factor * self.precision
    }

    /// This is the format used to pass along to HTTP solver.
    pub fn as_big_rational(&self) -> BigRational {
        // We can assert that the precision is non-zero as we check when constructing
        // new `AmplificationParameter` instances that this invariant holds, and we don't
        // allow modifications of `self.precision` such that it could become 0.
        debug_assert!(!self.precision.is_zero());
        BigRational::new(self.factor.to_big_int(), self.precision.to_big_int())
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2StablePoolFactory {
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
        let pool_contract =
            BalancerV2StablePool::at(&self.raw_instance().web3(), pool_info.common.address);

        let amplification_parameter = pool_contract
            .get_amplification_parameter()
            .block(block)
            .batch_call(batch);

        async move {
            let common = common_pool_state.await;
            let amplification_parameter = {
                let (factor, _, precision) = amplification_parameter.await?;
                AmplificationParameter::new(factor, precision)?
            };

            Ok(Some(PoolState {
                tokens: common.tokens,
                swap_fee: common.swap_fee,
                amplification_parameter,
            }))
        }
        .boxed()
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2StablePoolFactoryV2 {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        as_v1(self).specialize_pool_info(pool).await
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        as_v1(self).fetch_pool_state(pool_info, common_pool_state, batch, block)
    }
}

fn as_v1(factory: &BalancerV2StablePoolFactoryV2) -> BalancerV2StablePoolFactory {
    BalancerV2StablePoolFactory::at(&factory.raw_instance().web3(), factory.address())
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
            H160([1; 20]) => common::TokenState {
                balance: bfp!("1000.0").as_uint256(),
                scaling_exponent: 0,
            },
            H160([2; 20]) => common::TokenState {
                balance: bfp!("10.0").as_uint256(),
                scaling_exponent: 0,
            },
            H160([3; 20]) => common::TokenState {
                balance: 15_000_000.into(),
                scaling_exponent: 12,
            },
        };
        let swap_fee = bfp!("0.00015");
        let amplification_parameter =
            AmplificationParameter::new(200.into(), 10000.into()).unwrap();

        let mock = Mock::new(42);
        let web3 = mock.web3();

        let pool = mock.deploy(BalancerV2StablePool::raw_contract().abi.clone());
        pool.expect_call(BalancerV2StablePool::signatures().get_amplification_parameter())
            .returns((
                amplification_parameter.factor,
                false,
                amplification_parameter.precision,
            ));

        let factory = dummy_contract!(BalancerV2StablePoolFactory, H160::default());
        let pool_info = PoolInfo {
            common: common::PoolInfo {
                id: H256([0x90; 32]),
                address: pool.address(),
                tokens: tokens.keys().copied().collect(),
                scaling_exponents: tokens
                    .values()
                    .map(|token| token.scaling_exponent)
                    .collect(),
                block_created: (1337, H256::from_low_u64_be(1337)),
            },
        };
        let common_pool_state = common::PoolState {
            paused: false,
            swap_fee,
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

        assert_eq!(
            pool_state,
            Some(PoolState {
                tokens: common_pool_state.tokens,
                swap_fee,
                amplification_parameter,
            })
        );
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

        assert!(PoolInfo::from_graph_data(&pool, (42, H256::from_low_u64_be(42))).is_err());
    }

    #[test]
    fn amplification_parameter_conversions() {
        assert_eq!(
            AmplificationParameter::new(2.into(), 3.into())
                .unwrap()
                .as_u256(),
            6.into()
        );
        assert_eq!(
            AmplificationParameter::new(7.into(), 8.into())
                .unwrap()
                .as_big_rational(),
            BigRational::new(7.into(), 8.into())
        );

        assert_eq!(
            AmplificationParameter::new(1.into(), 0.into())
                .unwrap_err()
                .to_string(),
            "Zero precision not allowed"
        );
    }
}
