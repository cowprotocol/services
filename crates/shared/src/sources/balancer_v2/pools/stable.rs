//! Module implementing stable pool specific indexing logic.

use {
    super::{FactoryIndexing, PoolIndexing, common},
    crate::{
        conversions::U256Ext as _,
        sources::balancer_v2::{
            graph_api::{PoolData, PoolType},
            swap::fixed_point::Bfp,
        },
    },
    alloy::primitives::{Address, U256},
    anyhow::{Result, ensure},
    contracts::alloy::{BalancerV2StablePool, BalancerV2StablePoolFactoryV2},
    ethcontract::BlockId,
    ethrpc::alloy::conversions::IntoAlloy,
    futures::{FutureExt as _, future::BoxFuture},
    num::BigRational,
    std::collections::BTreeMap,
};

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
    pub tokens: BTreeMap<Address, common::TokenState>,
    pub swap_fee: Bfp,
    pub amplification_parameter: AmplificationParameter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmplificationParameter {
    factor: U256,
    precision: U256,
}

impl AmplificationParameter {
    pub fn try_new(factor: U256, precision: U256) -> Result<Self> {
        ensure!(!precision.is_zero(), "Zero precision not allowed");
        Ok(Self { factor, precision })
    }

    /// This is the format used to pass into smart contracts.
    pub fn with_base(&self, base: U256) -> Option<U256> {
        Some(self.factor.checked_mul(base)? / self.precision)
    }

    /// This is the format used to pass along to HTTP solver.
    pub fn as_big_rational(&self) -> BigRational {
        // We can assert that the precision is non-zero as we check when constructing
        // new `AmplificationParameter` instances that this invariant holds, and we
        // don't allow modifications of `self.precision` such that it could
        // become 0.
        debug_assert!(!self.precision.is_zero());
        BigRational::new(self.factor.to_big_int(), self.precision.to_big_int())
    }

    pub fn factor(&self) -> U256 {
        self.factor
    }

    pub fn precision(&self) -> U256 {
        self.precision
    }
}

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2StablePoolFactoryV2::Instance {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        Ok(PoolInfo { common: pool })
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        let pool_contract =
            BalancerV2StablePool::Instance::new(pool_info.common.address, self.provider().clone());

        let fetch_common = common_pool_state.map(Result::Ok);
        let fetch_amplification_parameter = async move {
            pool_contract
                .getAmplificationParameter()
                .block(block.into_alloy())
                .call()
                .await
                .map_err(anyhow::Error::from)
        };

        async move {
            let (common, amplification_parameter) =
                futures::try_join!(fetch_common, fetch_amplification_parameter)?;
            let amplification_parameter = {
                AmplificationParameter::try_new(
                    amplification_parameter.value,
                    amplification_parameter.precision,
                )?
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

#[cfg(test)]
mod tests {
    use {super::*, crate::sources::balancer_v2::graph_api::Token, ethcontract::H256};

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: H256([2; 32]),
            address: Address::repeat_byte(1),
            factory: Address::repeat_byte(0xfa),
            swap_enabled: true,
            tokens: vec![
                Token {
                    address: Address::repeat_byte(0x11),
                    decimals: 1,
                    weight: None,
                },
                Token {
                    address: Address::repeat_byte(0x22),
                    decimals: 2,
                    weight: None,
                },
            ],
        };

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[test]
    fn amplification_parameter_conversions() {
        assert_eq!(
            AmplificationParameter::try_new(U256::from(2u64), U256::from(3u64))
                .unwrap()
                .with_base(U256::from(1000u64))
                .unwrap(),
            U256::from(666u64)
        );
        assert_eq!(
            AmplificationParameter::try_new(U256::from(7u64), U256::from(8u64))
                .unwrap()
                .as_big_rational(),
            BigRational::new(7.into(), 8.into())
        );

        assert_eq!(
            AmplificationParameter::try_new(U256::from(1u64), U256::from(0u64))
                .unwrap_err()
                .to_string(),
            "Zero precision not allowed"
        );
    }
}
