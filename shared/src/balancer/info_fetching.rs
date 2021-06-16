//! Responsible for conversion of a `pool_address` into `WeightedPoolInfo` which is used by the
//! event handler to construct a `RegisteredWeightedPool`.
use crate::pool_fetching::MAX_BATCH_SIZE;
use crate::token_info::TokenInfoFetching;
use crate::Web3;
use anyhow::{anyhow, Result};
use contracts::{BalancerV2Vault, BalancerV2WeightedPool};
use ethcontract::batch::CallBatch;
use ethcontract::{Bytes, H160, H256, U256};
use mockall::*;
use std::sync::Arc;

#[derive(Clone)]
pub struct WeightedPoolInfo {
    pub pool_id: H256,
    pub tokens: Vec<H160>,
    pub weights: Vec<U256>,
    pub scaling_exponents: Vec<u8>,
}

/// Via `PoolInfoFetcher` (leverages a combination of `Web3` and `TokenInfoFetching`)
/// to recover `WeightedPoolInfo` from a `pool_address` in steps as follows:
/// 1. The `pool_id` is recovered first from the deployed `BalancerV2Vault` contract.
/// 2. With `pool_id` we can BatchCall for `tokens` (just the addresses) and `normalized_weights`
///     Technically, `normalized_weights` could be queried along with `pool_id` in step 1,
///     but batching here or there doesn't make a difference.
/// 3. Finally, the `scaling_exponents` are derived as 18 - decimals (for each the token in the pool)
///     `TokenInfoFetching` is used here since it is optimized for recovering ERC20 info internally.
///
/// Note that all token decimals are required to be returned from `TokenInfoFetching` in order
/// to accurately construct `WeightedPoolInfo`.
pub struct PoolInfoFetcher {
    pub web3: Web3,
    pub token_info_fetcher: Arc<dyn TokenInfoFetching>,
}

#[automock]
#[async_trait::async_trait]
pub trait PoolInfoFetching: Send + Sync {
    async fn get_pool_data(&self, pool_address: H160) -> Result<WeightedPoolInfo>;
}

#[async_trait::async_trait]
impl PoolInfoFetching for PoolInfoFetcher {
    /// Could result in ethcontract::{NodeError, MethodError or ContractError}
    async fn get_pool_data(&self, pool_address: H160) -> Result<WeightedPoolInfo> {
        let mut batch = CallBatch::new(self.web3.transport());
        let pool_contract = BalancerV2WeightedPool::at(&self.web3, pool_address);
        // Need vault and pool_id before we can fetch tokens.
        let vault = BalancerV2Vault::deployed(&self.web3).await?;
        let pool_id = H256::from(pool_contract.methods().get_pool_id().call().await?.0);

        // token_data and weight calls can be batched
        let token_data = vault
            .methods()
            .get_pool_tokens(Bytes(pool_id.0))
            .batch_call(&mut batch);
        let normalized_weights = pool_contract
            .methods()
            .get_normalized_weights()
            .batch_call(&mut batch);
        batch.execute_all(MAX_BATCH_SIZE).await;

        let tokens = token_data.await?.0;

        let token_decimals = self.token_info_fetcher.get_token_infos(&tokens).await;
        let ordered_decimals = tokens
            .iter()
            .map(|token| token_decimals.get(token).and_then(|t| t.decimals))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| anyhow!("all token decimals required to build scaling factors"))?;
        // Note that balancer does not support tokens with more than 18 decimals
        // https://github.com/balancer-labs/balancer-v2-monorepo/blob/ce70f7663e0ac94b25ed60cb86faaa8199fd9e13/pkg/pool-utils/contracts/BasePool.sol#L497-L508
        let scaling_exponents = ordered_decimals
            .iter()
            .map(|decimals| 18u8.checked_sub(*decimals))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| anyhow!("token with more than 18 decimals"))?;
        Ok(WeightedPoolInfo {
            pool_id,
            tokens,
            weights: normalized_weights.await?,
            scaling_exponents,
        })
    }
}
