//! Module implementing composable stable pool specific indexing logic.

use {
    super::{FactoryIndexing, PoolIndexing, common},
    crate::sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    anyhow::Result,
    contracts::bindings::{BalancerV2ComposableStablePool, BalancerV2ComposableStablePoolFactory},
    ethcontract::BlockId,
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    futures::{FutureExt as _, future::BoxFuture},
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
impl FactoryIndexing for BalancerV2ComposableStablePoolFactory::Instance {
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
        let pool_contract = BalancerV2ComposableStablePool::Instance::new(
            pool_info.common.address.into_alloy(),
            self.provider().clone(),
        );

        let fetch_common = common_pool_state.map(Result::Ok);
        let scaling_factors_block = block.into_alloy();
        let amp_param_block = scaling_factors_block;
        let pool_contract_clone = pool_contract.clone();
        let fetch_scaling_factors = async move {
            pool_contract
                .getScalingFactors()
                .block(scaling_factors_block)
                .call()
                .await
                .map_err(anyhow::Error::from)
        };
        let fetch_amplification_parameter = async move {
            pool_contract_clone
                .getAmplificationParameter()
                .block(amp_param_block)
                .call()
                .await
                .map_err(anyhow::Error::from)
        };

        async move {
            let (common, scaling_factors, amplification_parameter) = futures::try_join!(
                fetch_common,
                fetch_scaling_factors,
                fetch_amplification_parameter
            )?;
            let amplification_parameter = {
                AmplificationParameter::try_new(
                    amplification_parameter.value.into_legacy(),
                    amplification_parameter.precision.into_legacy(),
                )?
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
                                scaling_factor: Bfp::from_wei(scaling_factor.into_legacy()),
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
        ethcontract::{H160, H256},
    };

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
