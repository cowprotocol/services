//! Module implementing liquidity bootstrapping (with no protocol fees) pool
//! specific indexing logic.

use {
    super::{common, FactoryIndexing},
    crate::ethrpc::Web3CallBatch,
    anyhow::Result,
    contracts::{
        BalancerV2LiquidityBootstrappingPoolFactory,
        BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory,
    },
    ethcontract::BlockId,
    futures::future::BoxFuture,
};

pub use super::liquidity_bootstrapping::{PoolInfo, PoolState};

#[async_trait::async_trait]
impl FactoryIndexing for BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory {
    type PoolInfo = PoolInfo;
    type PoolState = PoolState;

    async fn specialize_pool_info(&self, pool: common::PoolInfo) -> Result<Self::PoolInfo> {
        as_liquidity_bootstrapping_factory(self)
            .specialize_pool_info(pool)
            .await
    }

    fn fetch_pool_state(
        &self,
        pool_info: &Self::PoolInfo,
        common_pool_state: BoxFuture<'static, common::PoolState>,
        batch: &mut Web3CallBatch,
        block: BlockId,
    ) -> BoxFuture<'static, Result<Option<Self::PoolState>>> {
        as_liquidity_bootstrapping_factory(self).fetch_pool_state(
            pool_info,
            common_pool_state,
            batch,
            block,
        )
    }
}

fn as_liquidity_bootstrapping_factory(
    factory: &BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory,
) -> BalancerV2LiquidityBootstrappingPoolFactory {
    BalancerV2LiquidityBootstrappingPoolFactory::at(
        &factory.raw_instance().web3(),
        factory.address(),
    )
}
