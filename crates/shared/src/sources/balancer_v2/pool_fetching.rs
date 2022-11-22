//! Pool Fetching is primarily concerned with retrieving relevant pools from the `BalancerPoolRegistry`
//! when given a collection of `TokenPair`. Each of these pools are then queried for
//! their `token_balances` and the `PoolFetcher` returns all up-to-date `Weighted` and `Stable`
//! pools to be consumed by external users (e.g. Price Estimators and Solvers).

mod aggregate;
mod cache;
mod internal;
mod pool_storage;
mod registry;

use self::{
    aggregate::Aggregate, cache::Cache, internal::InternalPoolFetching, registry::Registry,
};
use super::{
    graph_api::{BalancerSubgraphClient, RegisteredPools},
    pool_init::PoolInitializing,
    pools::{
        common::{self, PoolInfoFetcher},
        stable, weighted, FactoryIndexing, Pool, PoolIndexing, PoolKind,
    },
    swap::fixed_point::Bfp,
};
use crate::{
    current_block::{BlockRetrieving, CurrentBlockStream},
    ethrpc::{Web3, Web3Transport},
    maintenance::Maintaining,
    recent_block_cache::{Block, CacheConfig},
    token_info::TokenInfoFetching,
};
use anyhow::{Context, Result};
use clap::ValueEnum;
use contracts::{
    BalancerV2LiquidityBootstrappingPoolFactory,
    BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory, BalancerV2StablePoolFactory,
    BalancerV2StablePoolFactoryV2, BalancerV2Vault, BalancerV2WeightedPool2TokensFactory,
    BalancerV2WeightedPoolFactory,
};
use ethcontract::{dyns::DynInstance, BlockId, Instance, H160, H256};
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
    // We observed some balancer pools like https://app.balancer.fi/#/pool/0x072f14b85add63488ddad88f855fda4a99d6ac9b000200000000000000000027
    // being problematic because their token balance becomes out of sync leading to simulation
    // failures.
    // https://forum.balancer.fi/t/medium-severity-bug-found/3161
    pool_id_deny_list: Vec<H256>,
}

/// An enum containing all supported Balancer factory types.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum BalancerFactoryKind {
    Weighted,
    Weighted2Token,
    Stable,
    StableV2,
    LiquidityBootstrapping,
    NoProtocolFeeLiquidityBootstrapping,
}

impl BalancerFactoryKind {
    /// Returns a vector with supported factories for the specified chain ID.
    pub fn for_chain(chain_id: u64) -> Vec<Self> {
        match chain_id {
            1 => Self::value_variants().to_owned(),
            4 => vec![
                Self::Weighted,
                Self::Weighted2Token,
                Self::Stable,
                Self::LiquidityBootstrapping,
                Self::NoProtocolFeeLiquidityBootstrapping,
            ],
            5 => vec![Self::Weighted, Self::Weighted2Token],
            _ => Default::default(),
        }
    }
}

/// All balancer related contracts that we expect to exist.
pub struct BalancerContracts {
    pub vault: BalancerV2Vault,
    pub factories: HashMap<BalancerFactoryKind, DynInstance>,
}

impl BalancerContracts {
    pub async fn new(web3: &Web3, factory_kinds: Vec<BalancerFactoryKind>) -> Result<Self> {
        let vault = BalancerV2Vault::deployed(web3).await?;

        macro_rules! instance {
            ($factory:ident) => {{
                $factory::deployed(web3).await?.raw_instance().clone()
            }};
        }

        let mut factories = HashMap::new();
        for kind in factory_kinds {
            let instance = match &kind {
                BalancerFactoryKind::Weighted => instance!(BalancerV2WeightedPoolFactory),
                BalancerFactoryKind::Weighted2Token => {
                    instance!(BalancerV2WeightedPool2TokensFactory)
                }
                BalancerFactoryKind::Stable => instance!(BalancerV2StablePoolFactory),
                BalancerFactoryKind::StableV2 => instance!(BalancerV2StablePoolFactoryV2),
                BalancerFactoryKind::LiquidityBootstrapping => {
                    instance!(BalancerV2LiquidityBootstrappingPoolFactory)
                }
                BalancerFactoryKind::NoProtocolFeeLiquidityBootstrapping => {
                    instance!(BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory)
                }
            };

            factories.insert(kind, instance);
        }

        Ok(Self { vault, factories })
    }
}

impl BalancerPoolFetcher {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        chain_id: u64,
        block_retriever: Arc<dyn BlockRetrieving>,
        token_infos: Arc<dyn TokenInfoFetching>,
        config: CacheConfig,
        block_stream: CurrentBlockStream,
        client: Client,
        web3: Web3,
        contracts: &BalancerContracts,
        deny_listed_pool_ids: Vec<H256>,
    ) -> Result<Self> {
        let pool_initializer = BalancerSubgraphClient::for_chain(chain_id, client)?;
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

#[async_trait::async_trait]
impl Maintaining for BalancerPoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        self.fetcher.run_maintenance().await
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
        .eth()
        .block(BlockId::Number(fetched_block_number.into()))
        .await?
        .context("failed to get block by block number")?
        .hash
        .context("missing hash from block")?;
    let mut registered_pools_by_factory = registered_pools.group_by_factory();

    macro_rules! registry {
        ($factory:ident, $instance:expr) => {{
            create_internal_pool_fetcher(
                contracts.vault.clone(),
                $factory::with_deployment_info(
                    &$instance.web3(),
                    $instance.address(),
                    $instance.deployment_information(),
                ),
                block_retriever.clone(),
                token_infos.clone(),
                $instance,
                registered_pools_by_factory
                    .remove(&$instance.address())
                    .unwrap_or_else(|| RegisteredPools::empty(fetched_block_number)),
                fetched_block_hash,
            )?
        }};
    }

    let mut fetchers = Vec::new();
    for (kind, instance) in &contracts.factories {
        let registry = match kind {
            BalancerFactoryKind::Weighted => registry!(BalancerV2WeightedPoolFactory, instance),
            BalancerFactoryKind::Weighted2Token => {
                registry!(BalancerV2WeightedPool2TokensFactory, instance)
            }
            BalancerFactoryKind::Stable => registry!(BalancerV2StablePoolFactory, instance),
            BalancerFactoryKind::StableV2 => registry!(BalancerV2StablePoolFactoryV2, instance),
            BalancerFactoryKind::LiquidityBootstrapping => {
                registry!(BalancerV2LiquidityBootstrappingPoolFactory, instance)
            }
            BalancerFactoryKind::NoProtocolFeeLiquidityBootstrapping => {
                registry!(
                    BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory,
                    instance
                )
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
        tracing::warn!(
            %total_count, unused_pools = ?registered_pools_by_factory,
            "found pools that don't correspond to any known Balancer pool factory",
        );
    }

    Ok(Aggregate::new(fetchers))
}

/// Helper method for creating a boxed `InternalPoolFetching` instance for the
/// specified factory and parameters.
fn create_internal_pool_fetcher<Factory>(
    vault: BalancerV2Vault,
    factory: Factory,
    block_retriever: Arc<dyn BlockRetrieving>,
    token_infos: Arc<dyn TokenInfoFetching>,
    factory_instance: &Instance<Web3Transport>,
    registered_pools: RegisteredPools,
    fetched_block_hash: H256,
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
fn pool_address_from_id(pool_id: H256) -> H160 {
    let mut address = H160::default();
    address.0.copy_from_slice(&pool_id.0[..20]);
    address
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ethrpc,
        sources::balancer_v2::{
            graph_api::{BalancerSubgraphClient, PoolData, PoolType},
            pool_init::EmptyPoolInitializer,
        },
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    };
    use hex_literal::hex;
    use std::time::Duration;

    #[test]
    fn can_extract_address_from_pool_id() {
        assert_eq!(
            pool_address_from_id(H256(hex!(
                "36128d5436d2d70cab39c9af9cce146c38554ff0000200000000000000000009"
            ))),
            addr!("36128d5436d2d70cab39c9af9cce146c38554ff0"),
        );
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_pool_fetcher_print() {
        let transport = ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();
        let contracts =
            BalancerContracts::new(&web3, BalancerFactoryKind::value_variants().to_vec())
                .await
                .unwrap();
        let token_info_fetcher =
            Arc::new(CachedTokenInfoFetcher::new(Box::new(TokenInfoFetcher {
                web3: web3.clone(),
            })));
        let block_stream = crate::current_block::current_block_stream(
            Arc::new(web3.clone()),
            Duration::from_secs(1000),
        )
        .await
        .unwrap();
        let deny_list = vec![H256(hex_literal::hex!(
            "072f14b85add63488ddad88f855fda4a99d6ac9b000200000000000000000027"
        ))];
        // let deny_list = vec![];
        let pool_fetcher = BalancerPoolFetcher::new(
            chain_id,
            Arc::new(web3.clone()),
            token_info_fetcher,
            Default::default(),
            block_stream,
            Default::default(),
            web3,
            &contracts,
            deny_list,
        )
        .await
        .unwrap();
        pool_fetcher.run_maintenance().await.unwrap();
        let pair = TokenPair::new(
            addr!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            addr!("C011a73ee8576Fb46F5E1c5751cA3B9Fe0af2a6F"),
        )
        .unwrap();
        let fetched_pools_by_id = pool_fetcher
            .fetch_pools([pair].into_iter().collect(), Block::Recent)
            .await
            .unwrap()
            .into_iter()
            .map(|pool| (pool.id, pool))
            .collect::<HashMap<_, _>>();
        dbg!(fetched_pools_by_id.len());
    }

    #[tokio::test]
    #[ignore]
    async fn balancer_fetched_pools_match_subgraph() {
        let transport = ethrpc::create_env_test_transport();
        let web3 = Web3::new(transport);
        let chain_id = web3.eth().chain_id().await.unwrap().as_u64();

        println!("Indexing events for chain {}", chain_id);
        crate::tracing::initialize_for_tests("warn,shared=debug");

        let pool_initializer = EmptyPoolInitializer::for_chain(chain_id);
        let token_infos = TokenInfoFetcher { web3: web3.clone() };
        let contracts =
            BalancerContracts::new(&web3, BalancerFactoryKind::value_variants().to_vec())
                .await
                .unwrap();
        let pool_fetcher = BalancerPoolFetcher {
            fetcher: Arc::new(
                create_aggregate_pool_fetcher(
                    web3.clone(),
                    pool_initializer,
                    Arc::new(web3.clone()),
                    Arc::new(token_infos),
                    &contracts,
                )
                .await
                .unwrap(),
            ),
            pool_id_deny_list: Default::default(),
        };

        // index all the pools.
        pool_fetcher.run_maintenance().await.unwrap();

        // see what the subgraph says.
        let client = BalancerSubgraphClient::for_chain(chain_id, Client::new()).unwrap();
        let subgraph_pools = client.get_registered_pools().await.unwrap();
        let subgraph_token_pairs = subgraph_pools_token_pairs(&subgraph_pools.pools).collect();

        // fetch all pools and group them by ID.
        let fetched_pools_by_id = pool_fetcher
            .fetch_pools(
                subgraph_token_pairs,
                Block::Number(subgraph_pools.fetched_block_number),
            )
            .await
            .unwrap()
            .into_iter()
            .map(|pool| (pool.id, pool))
            .collect::<HashMap<_, _>>();

        let mut unknown_pools = Vec::new();
        for subgraph_pool in subgraph_pools.pools.iter().filter(|pool| pool.swap_enabled) {
            tracing::debug!(?subgraph_pool);

            let fetched_pool = match fetched_pools_by_id.get(&subgraph_pool.id) {
                Some(pool) => pool,
                None => {
                    unknown_pools.push(subgraph_pool.id);
                    continue;
                }
            };
            tracing::debug!(?fetched_pool);

            match &fetched_pool.kind {
                PoolKind::Weighted(state) => {
                    for token in &subgraph_pool.tokens {
                        let token_state = &state.tokens[&token.address];
                        assert_eq!(token_state.common.scaling_exponent, 18 - token.decimals);

                        // Don't check weights for LBPs because they may be out
                        // of date in the subgraph. See:
                        // <https://github.com/balancer-labs/balancer-subgraph-v2/issues/173>
                        if subgraph_pool.pool_type != PoolType::LiquidityBootstrapping {
                            assert_eq!(token_state.weight, token.weight.unwrap());
                        }
                    }
                }
                PoolKind::Stable(state) => {
                    for token in &subgraph_pool.tokens {
                        let token_state = &state.tokens[&token.address];
                        assert_eq!(token_state.scaling_exponent, 18 - token.decimals);
                    }
                }
            };
        }
        tracing::warn!(?unknown_pools);
    }

    fn subgraph_pools_token_pairs(pools: &[PoolData]) -> impl Iterator<Item = TokenPair> + '_ {
        pools.iter().flat_map(|pool| {
            let len = pool.tokens.len();
            (0..len)
                .flat_map(move |a| (a + 1..len).map(move |b| (a, b)))
                .filter_map(move |(a, b)| {
                    TokenPair::new(pool.tokens[a].address, pool.tokens[b].address)
                })
        })
    }
}
