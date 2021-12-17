//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `Weighted` and `Stable`
//! pools to be consumed by external users (e.g. Price Estimators and Solvers).

mod aggregate;
mod cache;
mod internal;
mod registry;

pub use self::cache::BalancerPoolCacheMetrics;
use self::{
    aggregate::Aggregate, cache::Cache, internal::InternalPoolFetching, registry::Registry,
};
use super::{
    pool_init::PoolInitializing,
    pools::{
        common::{self, PoolInfoFetcher},
        stable, weighted, PoolKind,
    },
    swap::fixed_point::Bfp,
};
use crate::{
    current_block::CurrentBlockStream,
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig},
    sources::balancer_v2::pool_init::SubgraphPoolInitializer,
    token_info::TokenInfoFetching,
    Web3,
};
use anyhow::Result;
use contracts::{
    BalancerV2StablePoolFactory, BalancerV2Vault, BalancerV2WeightedPool2TokensFactory,
    BalancerV2WeightedPoolFactory,
};
use ethcontract::{H160, H256};
use model::TokenPair;
use reqwest::Client;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub use common::TokenState;
pub use stable::AmplificationParameter;
pub use weighted::TokenState as WeightedTokenState;

pub trait BalancerPoolEvaluating {
    fn properties(&self) -> CommonPoolState;
}

#[derive(Clone, Debug)]
pub struct CommonPoolState {
    pub id: H256,
    pub address: H160,
    pub swap_fee: Bfp,
    pub paused: bool,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, WeightedTokenState>,
}

impl WeightedPool {
    pub fn new_unpaused(pool_id: H256, weighted_state: weighted::PoolState) -> Self {
        WeightedPool {
            common: CommonPoolState {
                id: pool_id,
                address: pool_address_from_id(pool_id),
                swap_fee: weighted_state.swap_fee,
                paused: false,
            },
            reserves: weighted_state.tokens.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StablePool {
    pub common: CommonPoolState,
    pub reserves: HashMap<H160, TokenState>,
    pub amplification_parameter: AmplificationParameter,
}

impl StablePool {
    pub fn new_unpaused(pool_id: H256, stable_state: stable::PoolState) -> Self {
        StablePool {
            common: CommonPoolState {
                id: pool_id,
                address: pool_address_from_id(pool_id),
                swap_fee: stable_state.swap_fee,
                paused: false,
            },
            reserves: stable_state.tokens.into_iter().collect(),
            amplification_parameter: stable_state.amplification_parameter,
        }
    }
}

#[derive(Default)]
pub struct FetchedBalancerPools {
    pub stable_pools: Vec<StablePool>,
    pub weighted_pools: Vec<WeightedPool>,
}

impl FetchedBalancerPools {
    pub fn relevant_tokens(&self) -> HashSet<H160> {
        let mut tokens = HashSet::new();
        tokens.extend(
            self.stable_pools
                .iter()
                .flat_map(|pool| pool.reserves.keys().copied()),
        );
        tokens.extend(
            self.weighted_pools
                .iter()
                .flat_map(|pool| pool.reserves.keys().copied()),
        );
        tokens
    }
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait BalancerPoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedBalancerPools>;
}

pub struct BalancerPoolFetcher {
    fetcher: Arc<dyn InternalPoolFetching>,
}

impl BalancerPoolFetcher {
    pub async fn new(
        chain_id: u64,
        web3: Web3,
        token_info_fetcher: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
        metrics: Arc<dyn BalancerPoolCacheMetrics>,
        client: Client,
    ) -> Result<Self> {
        let vault = BalancerV2Vault::deployed(&web3).await?;
        let weighted_pool_factory = BalancerV2WeightedPoolFactory::deployed(&web3).await?;
        let two_token_pool_factory = BalancerV2WeightedPool2TokensFactory::deployed(&web3).await?;
        let stable_pool_factory = BalancerV2StablePoolFactory::deployed(&web3).await?;

        let pool_initializer = SubgraphPoolInitializer::new(chain_id, client)?;
        let initial_pools = pool_initializer.initialize_pools().await?;
        let start_sync_at_block = Some(initial_pools.fetched_block_number);

        macro_rules! create_pool_registry {
            ($factory:expr, $initial_pools:expr) => {{
                let factory = $factory;
                let initial_pools = $initial_pools;
                Box::new(Registry::new(
                    Arc::new(PoolInfoFetcher::new(
                        vault.clone(),
                        factory.clone(),
                        token_info_fetcher.clone(),
                    )),
                    factory.raw_instance(),
                    initial_pools,
                    start_sync_at_block,
                ))
            }};
        }

        let fetcher = Arc::new(Cache::new(
            Aggregate::new(vec![
                create_pool_registry!(weighted_pool_factory, initial_pools.weighted_pools),
                create_pool_registry!(two_token_pool_factory, initial_pools.weighted_2token_pools),
                create_pool_registry!(stable_pool_factory, initial_pools.stable_pools),
            ]),
            config,
            block_stream,
            metrics,
        )?);

        Ok(Self { fetcher })
    }
}

#[async_trait::async_trait]
impl BalancerPoolFetching for BalancerPoolFetcher {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedBalancerPools> {
        let pool_ids = self.fetcher.pool_ids_for_token_pairs(token_pairs).await;
        let pools = self.fetcher.pools_by_id(pool_ids, at_block).await?;

        // For now, split the `Vec<Pool>` into a `FetchedBalancerPools` to keep
        // compatibility with the rest of the project. This should eventually
        // be removed and we should use `balancer_v2::pools::Pool` everywhere
        // instead.
        let fetched_pools = pools.into_iter().fold(
            FetchedBalancerPools::default(),
            |mut fetched_pools, pool| {
                match pool.kind {
                    PoolKind::Weighted(state) => fetched_pools
                        .weighted_pools
                        .push(WeightedPool::new_unpaused(pool.id, state)),
                    PoolKind::Stable(state) => fetched_pools
                        .stable_pools
                        .push(StablePool::new_unpaused(pool.id, state)),
                }
                fetched_pools
            },
        );

        Ok(fetched_pools)
    }
}

#[async_trait::async_trait]
impl Maintaining for BalancerPoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        self.fetcher.run_maintenance().await
    }
}

/// Extract the pool address from an ID.
///
/// This takes advantage that the first 20 bytes of the ID is the address of
/// the pool. For example the GNO-BAL pool with ID
/// `0x36128d5436d2d70cab39c9af9cce146c38554ff0000200000000000000000009`:
/// <https://etherscan.io/address/0x36128D5436d2d70cab39C9AF9CcE146C38554ff0>
fn pool_address_from_id(pool_id: H256) -> H160 {
    let mut address = H160::default();
    address.0.copy_from_slice(&pool_id.0[..20]);
    address
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    #[test]
    fn can_extract_address_from_pool_id() {
        assert_eq!(
            pool_address_from_id(H256(hex!(
                "36128d5436d2d70cab39c9af9cce146c38554ff0000200000000000000000009"
            ))),
            addr!("36128d5436d2d70cab39c9af9cce146c38554ff0"),
        );
    }
}
