//! Responsible for conversion of a `pool_address` into `*::PoolInfo` which is
//! stored by the event handler

use super::{
    pool_storage::{RegisteredStablePool, RegisteredWeightedPool},
    pools::{
        common::{self, PoolInfoFetching as _},
        FactoryIndexing as _,
    },
};
use crate::token_info::TokenInfoFetching;
use anyhow::Result;
use contracts::{BalancerV2StablePoolFactory, BalancerV2Vault, BalancerV2WeightedPoolFactory};
use ethcontract::H160;
use std::sync::Arc;

/// Legacy pool info fetching implementation.
///
/// This is used as an interim adapter between the new "split" token fetching
/// where each pool kind gets its dedicated module and the "legacy" way where
/// each pool gets its own method.
pub struct PoolInfoFetcher {
    inner: common::PoolInfoFetcher,
    weighted_factory: BalancerV2WeightedPoolFactory,
    stable_factory: BalancerV2StablePoolFactory,
}

impl PoolInfoFetcher {
    pub fn new(
        vault: BalancerV2Vault,
        token_infos: Arc<dyn TokenInfoFetching>,
        weighted_factory: BalancerV2WeightedPoolFactory,
        stable_factory: BalancerV2StablePoolFactory,
    ) -> Self {
        Self {
            inner: common::PoolInfoFetcher::new(vault, token_infos),
            weighted_factory,
            stable_factory,
        }
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait PoolInfoFetching: Send + Sync {
    async fn get_weighted_pool_data(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<RegisteredWeightedPool>;
    async fn get_stable_pool_data(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<RegisteredStablePool>;
}

#[async_trait::async_trait]
impl PoolInfoFetching for PoolInfoFetcher {
    /// Could result in ethcontract::{NodeError, MethodError or ContractError}
    async fn get_weighted_pool_data(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<RegisteredWeightedPool> {
        let common_info = self
            .inner
            .fetch_common_pool_info(pool_address, block_created)
            .await?;
        self.weighted_factory
            .specialize_pool_info(common_info)
            .await
    }

    async fn get_stable_pool_data(
        &self,
        pool_address: H160,
        block_created: u64,
    ) -> Result<RegisteredStablePool> {
        let common_info = self
            .inner
            .fetch_common_pool_info(pool_address, block_created)
            .await?;
        self.stable_factory.specialize_pool_info(common_info).await
    }
}
