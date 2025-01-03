//! Module implementing stable pool specific indexing logic.

use {
    super::{common, FactoryIndexing, PoolIndexing},
    crate::{
        conversions::U256Ext as _,
        sources::balancer_v2::{
            graph_api::{PoolData, PoolType},
            swap::fixed_point::Bfp,
        },
    },
    anyhow::{ensure, Result},
    contracts::{BalancerV2StablePool, BalancerV2StablePoolFactoryV2},
    ethcontract::{BlockId, H160, U256},
    futures::{future::BoxFuture, FutureExt as _},
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
    pub tokens: BTreeMap<H160, common::TokenState>,
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
impl FactoryIndexing for BalancerV2StablePoolFactoryV2 {
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
            BalancerV2StablePool::at(&self.raw_instance().web3(), pool_info.common.address);

        let fetch_common = common_pool_state.map(Result::Ok);
        let fetch_amplification_parameter = pool_contract
            .get_amplification_parameter()
            .block(block)
            .call();

        async move {
            let (common, amplification_parameter) =
                futures::try_join!(fetch_common, fetch_amplification_parameter)?;
            let amplification_parameter = {
                let (factor, _, precision) = amplification_parameter;
                AmplificationParameter::try_new(factor, precision)?
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
    use {
        super::*,
        crate::sources::balancer_v2::graph_api::Token,
        ethcontract::{H160, H256},
    };

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

        assert!(PoolInfo::from_graph_data(&pool, 42).is_err());
    }

    #[test]
    fn amplification_parameter_conversions() {
        assert_eq!(
            AmplificationParameter::try_new(2.into(), 3.into())
                .unwrap()
                .with_base(1000.into())
                .unwrap(),
            666.into()
        );
        assert_eq!(
            AmplificationParameter::try_new(7.into(), 8.into())
                .unwrap()
                .as_big_rational(),
            BigRational::new(7.into(), 8.into())
        );

        assert_eq!(
            AmplificationParameter::try_new(1.into(), 0.into())
                .unwrap_err()
                .to_string(),
            "Zero precision not allowed"
        );
    }
}
