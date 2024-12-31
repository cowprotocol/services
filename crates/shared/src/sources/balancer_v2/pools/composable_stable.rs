//! Module implementing composable stable pool specific indexing logic.

use {
    super::{common, FactoryIndexing, PoolIndexing},
    crate::sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
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
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        let pool_contract = BalancerV2ComposableStablePool::at(
            &self.raw_instance().web3(),
            pool_info.common.address,
        );

        let fetch_common = common_pool_state.map(Result::Ok);
        let fetch_scaling_factors = pool_contract.get_scaling_factors().block(block).call();
        let fetch_amplification_parameter = pool_contract
            .get_amplification_parameter()
            .block(block)
            .call();

        async move {
            let (common, scaling_factors, amplification_parameter) = futures::try_join!(
                fetch_common,
                fetch_scaling_factors,
                fetch_amplification_parameter
            )?;
            let amplification_parameter = {
                let (factor, _, precision) = amplification_parameter;
                AmplificationParameter::try_new(factor, precision)?
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
