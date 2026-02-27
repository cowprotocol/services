//! Pool Fetching is primarily concerned with retrieving relevant pools from the
//! `BalancerPoolRegistry` when given a collection of `TokenPair`. Each of these
//! pools are then queried for their `token_balances` and the `PoolFetcher`
//! returns all up-to-date `Weighted` and `Stable` pools to be consumed by
//! external users (e.g. Price Estimators and Solvers).

use {
    self::{
        aggregate::Aggregate, cache::Cache, internal::InternalPoolFetching, registry::Registry,
    },
    super::{
        graph_api::{BalancerSubgraphClient, RegisteredPools},
        pool_init::PoolInitializing,
        pools::{
            FactoryIndexing, Pool, PoolIndexing, PoolKind,
            common::{self, PoolInfoFetcher},
            stable, weighted,
        },
        swap::fixed_point::Bfp,
    },
    crate::{
        recent_block_cache::{Block, CacheConfig},
        token_info::TokenInfoFetching,
        web3::Web3,
    },
    alloy::{
        eips::BlockNumberOrTag,
        primitives::{Address, B256},
        providers::{DynProvider, Provider},
    },
    anyhow::{Context, Result},
    contracts::{
        BalancerV2ComposableStablePoolFactory, BalancerV2ComposableStablePoolFactoryV3,
        BalancerV2ComposableStablePoolFactoryV4, BalancerV2ComposableStablePoolFactoryV5,
        BalancerV2ComposableStablePoolFactoryV6, BalancerV2LiquidityBootstrappingPoolFactory,
        BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory, BalancerV2StablePoolFactoryV2,
        BalancerV2Vault, BalancerV2WeightedPool2TokensFactory, BalancerV2WeightedPoolFactory,
        BalancerV2WeightedPoolFactoryV3, BalancerV2WeightedPoolFactoryV4,
    },
    ethrpc::{
        alloy::ProviderLabelingExt,
        block_stream::{BlockRetrieving, CurrentBlockWatcher},
    },
    model::TokenPair,
    reqwest::{Client, Url},
    std::{
        collections::{BTreeMap, HashSet},
        sync::Arc,
    },
    tracing::instrument,
};
pub use {
    common::TokenState,
    stable::AmplificationParameter,
    weighted::{TokenState as WeightedTokenState, Version as WeightedPoolVersion},
};

mod aggregate;
mod cache;
mod internal;
mod pool_storage;
mod registry;

pub trait BalancerPoolEvaluating {
    fn properties(&self) -> CommonPoolState;
}

#[derive(Clone, Debug)]
pub struct CommonPoolState {
    pub id: B256,
    pub address: Address,
    pub swap_fee: Bfp,
    pub paused: bool,
}

#[derive(Clone, Debug)]
pub struct WeightedPool {
    pub common: CommonPoolState,
    pub reserves: BTreeMap<Address, WeightedTokenState>,
    pub version: WeightedPoolVersion,
}

impl WeightedPool {
    pub fn new_unpaused(pool_id: B256, weighted_state: weighted::PoolState) -> Self {
        WeightedPool {
            common: CommonPoolState {
                id: pool_id,
                address: pool_address_from_id(pool_id),
                swap_fee: weighted_state.swap_fee,
                paused: false,
            },
            reserves: weighted_state.tokens.into_iter().collect(),
            version: weighted_state.version,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StablePool {
    pub common: CommonPoolState,
    pub reserves: BTreeMap<Address, TokenState>,
    pub amplification_parameter: AmplificationParameter,
}

impl StablePool {
    pub fn new_unpaused(pool_id: B256, stable_state: stable::PoolState) -> Self {
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
    pub fn relevant_tokens(&self) -> HashSet<Address> {
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

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
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
    // We observed some balancer pools like https://app.balancer.fi/#/pool/0x072f14b85add63488ddad88f855fda4a99d6ac9b000200000000000000000027
    // being problematic because their token balance becomes out of sync leading to simulation
    // failures.
    // https://forum.balancer.fi/t/medium-severity-bug-found/3161
    pool_id_deny_list: Vec<B256>,
}

pub enum BalancerFactoryInstance {
    Weighted(BalancerV2WeightedPoolFactory::Instance),
    WeightedV3(BalancerV2WeightedPoolFactoryV3::Instance),
    WeightedV4(BalancerV2WeightedPoolFactoryV4::Instance),
    Weighted2Token(BalancerV2WeightedPool2TokensFactory::Instance),
    StableV2(BalancerV2StablePoolFactoryV2::Instance),
    LiquidityBootstrapping(BalancerV2LiquidityBootstrappingPoolFactory::Instance),
    NoProtocolFeeLiquidityBootstrapping(
        BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory::Instance,
    ),
    ComposableStable(BalancerV2ComposableStablePoolFactory::Instance),
    ComposableStableV3(BalancerV2ComposableStablePoolFactoryV3::Instance),
    ComposableStableV4(BalancerV2ComposableStablePoolFactoryV4::Instance),
    ComposableStableV5(BalancerV2ComposableStablePoolFactoryV5::Instance),
    ComposableStableV6(BalancerV2ComposableStablePoolFactoryV6::Instance),
}

impl BalancerFactoryInstance {
    pub fn address(&self) -> &alloy::primitives::Address {
        match self {
            BalancerFactoryInstance::Weighted(instance) => instance.address(),
            BalancerFactoryInstance::WeightedV3(instance) => instance.address(),
            BalancerFactoryInstance::WeightedV4(instance) => instance.address(),
            BalancerFactoryInstance::Weighted2Token(instance) => instance.address(),
            BalancerFactoryInstance::StableV2(instance) => instance.address(),
            BalancerFactoryInstance::LiquidityBootstrapping(instance) => instance.address(),
            BalancerFactoryInstance::NoProtocolFeeLiquidityBootstrapping(instance) => {
                instance.address()
            }
            BalancerFactoryInstance::ComposableStable(instance) => instance.address(),
            BalancerFactoryInstance::ComposableStableV3(instance) => instance.address(),
            BalancerFactoryInstance::ComposableStableV4(instance) => instance.address(),
            BalancerFactoryInstance::ComposableStableV5(instance) => instance.address(),
            BalancerFactoryInstance::ComposableStableV6(instance) => instance.address(),
        }
    }

    pub fn provider(&self) -> &DynProvider {
        match self {
            BalancerFactoryInstance::Weighted(instance) => instance.provider(),
            BalancerFactoryInstance::WeightedV3(instance) => instance.provider(),
            BalancerFactoryInstance::WeightedV4(instance) => instance.provider(),
            BalancerFactoryInstance::Weighted2Token(instance) => instance.provider(),
            BalancerFactoryInstance::StableV2(instance) => instance.provider(),
            BalancerFactoryInstance::LiquidityBootstrapping(instance) => instance.provider(),
            BalancerFactoryInstance::NoProtocolFeeLiquidityBootstrapping(instance) => {
                instance.provider()
            }
            BalancerFactoryInstance::ComposableStable(instance) => instance.provider(),
            BalancerFactoryInstance::ComposableStableV3(instance) => instance.provider(),
            BalancerFactoryInstance::ComposableStableV4(instance) => instance.provider(),
            BalancerFactoryInstance::ComposableStableV5(instance) => instance.provider(),
            BalancerFactoryInstance::ComposableStableV6(instance) => instance.provider(),
        }
    }
}

/// All balancer related contracts that we expect to exist.
pub struct BalancerContracts {
    pub vault: BalancerV2Vault::Instance,
    pub factories: Vec<BalancerFactoryInstance>,
}

impl BalancerPoolFetcher {
    #[expect(clippy::too_many_arguments)]
    pub async fn new(
        subgraph_url: &Url,
        block_retriever: Arc<dyn BlockRetrieving>,
        token_infos: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockWatcher,
        client: Client,
        web3: Web3,
        contracts: &BalancerContracts,
        deny_listed_pool_ids: Vec<B256>,
    ) -> Result<Self> {
        let pool_initializer = BalancerSubgraphClient::from_subgraph_url(subgraph_url, client)?;
        let web3 = web3.labeled("balancerV2");
        let fetcher = Arc::new(Cache::new(
            create_aggregate_pool_fetcher(
                web3,
                pool_initializer,
                block_retriever,
                token_infos,
                contracts,
            )
            .await?,
            config,
            block_stream,
        )?);

        Ok(Self {
            fetcher,
            pool_id_deny_list: deny_listed_pool_ids,
        })
    }

    async fn fetch_pools(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Pool>> {
        let mut pool_ids = self.fetcher.pool_ids_for_token_pairs(token_pairs).await;
        for id in &self.pool_id_deny_list {
            pool_ids.remove(id);
        }
        let pools = self.fetcher.pools_by_id(pool_ids, at_block).await?;

        Ok(pools)
    }
}

#[async_trait::async_trait]
impl BalancerPoolFetching for BalancerPoolFetcher {
    #[instrument(skip_all)]
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<FetchedBalancerPools> {
        let pools = self.fetch_pools(token_pairs, at_block).await?;

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

/// Creates an aggregate fetcher for all supported pool factories.
async fn create_aggregate_pool_fetcher(
    web3: Web3,
    pool_initializer: impl PoolInitializing,
    block_retriever: Arc<dyn BlockRetrieving>,
    token_infos: Arc<dyn TokenInfoFetching>,
    contracts: &BalancerContracts,
) -> Result<Aggregate> {
    let registered_pools = pool_initializer.initialize_pools().await?;
    let fetched_block_number = registered_pools.fetched_block_number;
    let fetched_block_hash = web3
        .provider
        .get_block_by_number(BlockNumberOrTag::Number(fetched_block_number))
        .await?
        .context("failed to get block by block number")?
        .hash();
    let mut registered_pools_by_factory = registered_pools.group_by_factory();

    macro_rules! registry {
        ($factory:ident, $instance:expr_2021) => {{
            create_internal_pool_fetcher(
                contracts.vault.clone(),
                $factory::Instance::new(*$instance.address(), $instance.provider().clone()),
                block_retriever.clone(),
                token_infos.clone(),
                $instance,
                registered_pools_by_factory
                    .remove(&(*$instance.address()))
                    .unwrap_or_else(|| RegisteredPools::empty(fetched_block_number)),
                fetched_block_hash,
            )?
        }};
    }

    let mut fetchers = Vec::new();
    for instance in &contracts.factories {
        let registry = match &instance {
            BalancerFactoryInstance::Weighted(_) => {
                registry!(BalancerV2WeightedPoolFactory, instance)
            }
            BalancerFactoryInstance::Weighted2Token(_) => {
                registry!(BalancerV2WeightedPoolFactory, instance)
            }
            BalancerFactoryInstance::WeightedV3(_) => {
                registry!(BalancerV2WeightedPoolFactoryV3, instance)
            }
            BalancerFactoryInstance::WeightedV4(_) => {
                registry!(BalancerV2WeightedPoolFactoryV3, instance)
            }
            BalancerFactoryInstance::StableV2(_) => {
                registry!(BalancerV2StablePoolFactoryV2, instance)
            }
            BalancerFactoryInstance::LiquidityBootstrapping(_) => {
                registry!(BalancerV2LiquidityBootstrappingPoolFactory, instance)
            }
            BalancerFactoryInstance::NoProtocolFeeLiquidityBootstrapping(_) => {
                registry!(BalancerV2LiquidityBootstrappingPoolFactory, instance)
            }
            BalancerFactoryInstance::ComposableStable(_) => {
                registry!(BalancerV2ComposableStablePoolFactory, instance)
            }
            BalancerFactoryInstance::ComposableStableV3(_) => {
                registry!(BalancerV2ComposableStablePoolFactory, instance)
            }
            BalancerFactoryInstance::ComposableStableV4(_) => {
                registry!(BalancerV2ComposableStablePoolFactory, instance)
            }
            BalancerFactoryInstance::ComposableStableV5(_) => {
                registry!(BalancerV2ComposableStablePoolFactory, instance)
            }
            BalancerFactoryInstance::ComposableStableV6(_) => {
                registry!(BalancerV2ComposableStablePoolFactory, instance)
            }
        };
        fetchers.push(registry);
    }

    // Just to catch cases where new Balancer factories get added for a pool
    // kind, but we don't index it, log a warning for unused pools.
    if !registered_pools_by_factory.is_empty() {
        let total_count = registered_pools_by_factory
            .values()
            .map(|registered| registered.pools.len())
            .sum::<usize>();
        let factories = registered_pools_by_factory
            .keys()
            .copied()
            .collect::<Vec<_>>();
        tracing::warn!(
            %total_count, ?factories,
            "found pools that don't correspond to any known Balancer pool factory",
        );
    }

    Ok(Aggregate::new(fetchers))
}

/// Helper method for creating a boxed `InternalPoolFetching` instance for the
/// specified factory and parameters.
fn create_internal_pool_fetcher<Factory>(
    vault: BalancerV2Vault::Instance,
    factory: Factory,
    block_retriever: Arc<dyn BlockRetrieving>,
    token_infos: Arc<dyn TokenInfoFetching>,
    factory_instance: &BalancerFactoryInstance,
    registered_pools: RegisteredPools,
    fetched_block_hash: B256,
) -> Result<Box<dyn InternalPoolFetching>>
where
    Factory: FactoryIndexing,
{
    let initial_pools = registered_pools
        .pools
        .iter()
        .map(|pool| Factory::PoolInfo::from_graph_data(pool, registered_pools.fetched_block_number))
        .collect::<Result<_>>()?;
    let start_sync_at_block = Some((registered_pools.fetched_block_number, fetched_block_hash));

    Ok(Box::new(Registry::new(
        block_retriever,
        Arc::new(PoolInfoFetcher::new(vault, factory, token_infos)),
        factory_instance,
        initial_pools,
        start_sync_at_block,
    )))
}

/// Extract the pool address from an ID.
///
/// This takes advantage that the first 20 bytes of the ID is the address of
/// the pool. For example the GNO-BAL pool with ID
/// `0x36128d5436d2d70cab39c9af9cce146c38554ff0000200000000000000000009`:
/// <https://etherscan.io/address/0x36128D5436d2d70cab39C9AF9CcE146C38554ff0>
fn pool_address_from_id(pool_id: B256) -> Address {
    Address::from_slice(&pool_id.as_slice()[..20])
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{address, b256},
    };

    #[test]
    fn can_extract_address_from_pool_id() {
        assert_eq!(
            pool_address_from_id(b256!(
                "36128d5436d2d70cab39c9af9cce146c38554ff0000200000000000000000009"
            )),
            address!("36128d5436d2d70cab39c9af9cce146c38554ff0"),
        );
    }
}
