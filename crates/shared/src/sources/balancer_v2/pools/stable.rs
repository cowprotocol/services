//! Module implementing stable pool specific indexing logic.

use super::{common, FactoryIndexing, PoolIndexing};
use crate::{
    conversions::U256Ext as _,
    sources::balancer_v2::graph_api::{PoolData, PoolType},
};
use anyhow::{ensure, Result};
use contracts::{BalancerV2StablePool, BalancerV2StablePoolFactory};
use ethcontract::{BlockId, H160, U256};
use futures::{future::BoxFuture, FutureExt as _};
use num::BigRational;
use std::collections::BTreeMap;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolState {
    pub tokens: BTreeMap<H160, common::TokenState>,
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

    async fn augment_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        Ok(PoolInfo { common: pool })
    }

    fn fetch_pool_state(
        &self,
        pool_info: Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut crate::Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<super::PoolState>> {
        let pool_contract =
            BalancerV2StablePool::at(&self.raw_instance().web3(), pool_info.common.address);

        let amplification_parameter = pool_contract
            .get_amplification_parameter()
            .block(block)
            .batch_call(batch);

        async move {
            let tokens = common_pool_state.await.tokens;
            let amplification_parameter = {
                let (factor, _, precision) = amplification_parameter.await?;
                AmplificationParameter::new(factor, precision)?
            };

            Ok(super::PoolState::Stable(PoolState {
                tokens,
                amplification_parameter,
            }))
        }
        .boxed()
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
