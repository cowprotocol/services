//! Module implementing liquidity bootstrapping pool specific indexing logic.

use {
    super::{FactoryIndexing, PoolIndexing, common},
    crate::sources::balancer_v2::{
        graph_api::{PoolData, PoolType},
        swap::fixed_point::Bfp,
    },
    alloy::eips::BlockId,
    anyhow::Result,
    contracts::{
        BalancerV2LiquidityBootstrappingPool, BalancerV2LiquidityBootstrappingPoolFactory,
    },
    futures::{FutureExt as _, future::BoxFuture},
};

pub use super::weighted::{PoolState, TokenState, Version};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PoolInfo {
    pub common: common::PoolInfo,
}

impl PoolIndexing for PoolInfo {
    fn from_graph_data(pool: &PoolData, block_created: u64) -> Result<Self> {
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
impl FactoryIndexing for BalancerV2LiquidityBootstrappingPoolFactory::Instance {
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
        let pool_contract = BalancerV2LiquidityBootstrappingPool::Instance::new(
            pool_info.common.address,
            self.provider().clone(),
        );

        let fetch_common = common_pool_state.map(Result::Ok);
        // Liquidity bootstrapping pools use dynamic weights, meaning that we
        // need to fetch them every time.
        let weights_block = block;
        let swap_block = weights_block;
        let pool_contract_clone = pool_contract.clone();
        let fetch_weights = async move {
            pool_contract
                .getNormalizedWeights()
                .block(weights_block)
                .call()
                .await
                .map_err(anyhow::Error::from)
        };
        let fetch_swap_enabled = async move {
            pool_contract_clone
                .getSwapEnabled()
                .block(swap_block)
                .call()
                .await
                .map_err(anyhow::Error::from)
        };

        async move {
            let (common, weights, swap_enabled) =
                futures::try_join!(fetch_common, fetch_weights, fetch_swap_enabled)?;
            if !swap_enabled {
                return Ok(None);
            }

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

            Ok(Some(PoolState {
                tokens,
                swap_fee,
                version: Version::V0,
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
        alloy::primitives::{Address, B256},
    };

    #[test]
    fn errors_when_converting_wrong_pool_type() {
        let pool = PoolData {
            pool_type: PoolType::Weighted,
            id: B256::repeat_byte(2),
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
}
