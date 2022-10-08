use crate::current_block::RangeInclusive;
use crate::event_handling::{EventHandler, EventStoring, MAX_REORG_BLOCK_COUNT};
use crate::maintenance::Maintaining;
use crate::recent_block_cache::Block;
use crate::Web3;

use super::event_fetching::{RecentEventsCache, UniswapV3Event, UniswapV3PoolEventFetcher};
use super::graph_api::{PoolData, TickData, Token, UniV3SubgraphClient};
use anyhow::{Context, Result};
use ethcontract::{BlockNumber, Event, H160, U256};
use itertools::{Either, Itertools};
use model::{u256_decimal, TokenPair};
use num::{rational::Ratio, BigInt, Zero};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::collections::BTreeMap;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

const MAX_POOLS_TO_INITIALIZE: usize = 100;

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: &HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<PoolInfo>>;
}

/// Pool data in a format prepared for solvers.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolInfo {
    pub address: H160,
    pub tokens: Vec<Token>,
    pub state: PoolState,
    pub gas_stats: PoolStats,
}

/// Pool state in a format prepared for solvers.
#[serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolState {
    #[serde(with = "u256_decimal")]
    pub sqrt_price: U256,
    #[serde(with = "u256_decimal")]
    pub liquidity: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub tick: BigInt,
    // (tick_idx, liquidity_net)
    #[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")]
    pub liquidity_net: BTreeMap<BigInt, BigInt>,
    #[serde_as(as = "DisplayFromStr")]
    pub fee: Ratio<u32>,
}

/// Pool stats in a format prepared for solvers
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolStats {
    #[serde(with = "u256_decimal")]
    #[serde(rename = "mean")]
    pub mean_gas: U256,
}

impl TryFrom<PoolData> for PoolInfo {
    type Error = anyhow::Error;

    fn try_from(pool: PoolData) -> Result<Self> {
        Ok(Self {
            address: pool.id,
            tokens: vec![pool.token0, pool.token1],
            state: PoolState {
                sqrt_price: pool.sqrt_price,
                liquidity: pool.liquidity,
                tick: pool.tick,
                liquidity_net: pool
                    .ticks
                    .context("no ticks")?
                    .into_iter()
                    .filter_map(|tick| {
                        if tick.liquidity_net.is_zero() {
                            None
                        } else {
                            Some((tick.tick_idx, tick.liquidity_net))
                        }
                    })
                    .collect(),
                fee: Ratio::new(pool.fee_tier.as_u32(), 1_000_000u32),
            },
            gas_stats: PoolStats {
                mean_gas: U256::from(300_000), // todo: hardcoded for testing purposes
            },
        })
    }
}

#[derive(Default)]
struct PoolsCheckpoint {
    /// Pools state.
    pools: HashMap<H160, PoolData>,
    /// Block number for which `pools` field was populated.
    block_number: u64,
    /// Pools that don't exist in `pools` field, therefore need to be initialized and moved to `pools`
    /// in the next maintainance run
    missing_pools: HashSet<H160>,
}

struct PoolsCheckpointHandler {
    graph_api: UniV3SubgraphClient,
    /// H160 is pool id while TokenPair is a pair or tokens for each pool.
    pools_by_token_pair: HashMap<TokenPair, HashSet<H160>>,
    /// Pools state on a specific block number in history considered reorg safe
    pools_checkpoint: Mutex<PoolsCheckpoint>,
}

impl PoolsCheckpointHandler {
    /// Fetches the list of existing UniswapV3 pools and their metadata (without state/ticks).
    /// Then fetches state/ticks for the most deepest pools (subset of all existing pools)
    pub async fn new(chain_id: u64, client: Client) -> Result<Self> {
        let graph_api = UniV3SubgraphClient::for_chain(chain_id, client)?;
        let mut registered_pools = graph_api.get_registered_pools().await?;
        tracing::debug!(
            block = %registered_pools.fetched_block_number, pools = %registered_pools.pools.len(),
            "initialized registered pools",
        );

        let mut pools_by_token_pair: HashMap<TokenPair, HashSet<H160>> = HashMap::new();
        for pool in &registered_pools.pools {
            let pair =
                TokenPair::new(pool.token0.id, pool.token1.id).context("cant create pair")?;
            pools_by_token_pair.entry(pair).or_default().insert(pool.id);
        }

        // can't fetch the state of all pools in constructor for performance reasons,
        // so let's fetch the top MAX_POOLS_TO_INITIALIZE pools with the highest liquidity
        registered_pools.pools.sort_unstable_by(|a, b| {
            a.total_value_locked_eth
                .partial_cmp(&b.total_value_locked_eth)
                .unwrap()
        });
        let pool_ids = registered_pools
            .pools
            .clone()
            .into_iter()
            .map(|pool| pool.id)
            .rev()
            .take(MAX_POOLS_TO_INITIALIZE)
            .collect::<Vec<_>>();
        let pools = graph_api
            .get_pools_with_ticks_by_ids(&pool_ids, registered_pools.fetched_block_number)
            .await?
            .into_iter()
            .map(|pool| (pool.id, pool))
            .collect::<HashMap<_, _>>();
        let pools_checkpoint = Mutex::new(PoolsCheckpoint {
            pools,
            block_number: registered_pools.fetched_block_number,
            ..Default::default()
        });

        Ok(Self {
            graph_api,
            pools_by_token_pair,
            pools_checkpoint,
        })
    }

    /// For a given list of token pairs, fetches the pools for the ones that exist in the checkpoint.
    /// For the ones that don't exist, flag as missing and expect to exist after the next maintenance run.
    fn get(&self, token_pairs: &HashSet<TokenPair>) -> (HashMap<H160, PoolData>, u64) {
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        tracing::debug!("get checkpoint for pool_ids: {:?}", pool_ids);

        match pool_ids.peek() {
            Some(_) => {
                let mut pools_checkpoint = self.pools_checkpoint.lock().unwrap();
                let (existing_pools, missing_pools): (HashMap<H160, PoolData>, Vec<H160>) =
                    pool_ids.partition_map(|pool_id| match pools_checkpoint.pools.get(pool_id) {
                        Some(entry) => Either::Left((entry.id, entry.clone())),
                        _ => Either::Right(pool_id),
                    });
                tracing::debug!(
                    "cache hit: {:?}, cache miss: {:?}",
                    existing_pools,
                    missing_pools
                );
                pools_checkpoint
                    .missing_pools
                    .extend(missing_pools.into_iter());
                (existing_pools, pools_checkpoint.block_number)
            }
            None => Default::default(),
        }
    }

    /// Fetches state/ticks for missing pools and moves them from `missing_pools` to `pools`
    async fn update_missing_pools(&self) -> Result<()> {
        let (missing_pools, block_number) = {
            let checkpoint = self.pools_checkpoint.lock().unwrap();
            (checkpoint.missing_pools.clone(), checkpoint.block_number)
        };

        let pool_ids = missing_pools.into_iter().collect::<Vec<_>>();
        let pools = self
            .graph_api
            .get_pools_with_ticks_by_ids(&pool_ids, block_number)
            .await?;

        let mut checkpoint = self.pools_checkpoint.lock().unwrap();
        for pool in pools {
            checkpoint.missing_pools.remove(&pool.id);
            checkpoint.pools.insert(pool.id, pool);
        }
        Ok(())
    }
}

pub struct UniswapV3PoolFetcher {
    /// Pools state on a specific block number in history considered reorg safe
    checkpoint: PoolsCheckpointHandler,
    /// Recent events used on top of pools_checkpoint to get the `latest_block` pools state.
    events: tokio::sync::Mutex<EventHandler<Web3, UniswapV3PoolEventFetcher, RecentEventsCache>>,
}

impl UniswapV3PoolFetcher {
    pub async fn new(chain_id: u64, client: Client, web3: Web3) -> Result<Self> {
        let checkpoint = PoolsCheckpointHandler::new(chain_id, client).await?;

        let init_block = checkpoint.pools_checkpoint.lock().unwrap().block_number;
        let init_block = web3
            .eth()
            .block(BlockNumber::Number(init_block.into()).into())
            .await?
            .context("missing block for fetched block number")?;
        let init_block = (
            init_block
                .number
                .context("missing fetched block number")?
                .as_u64(),
            init_block.hash.context("missing fetched block hash")?,
        );

        let events = tokio::sync::Mutex::new(EventHandler::new(
            web3.clone(),
            UniswapV3PoolEventFetcher(web3.clone()),
            RecentEventsCache::default(),
            Some(init_block),
        ));

        Ok(Self { checkpoint, events })
    }

    /// Moves the checkpoint to the block `latest_block - MAX_REORG_BLOCK_COUNT`
    async fn move_checkpoint_to_future(&self) -> Result<()> {
        let last_event_block = self.events.lock().await.store().last_event_block().await?;
        let old_checkpoint_block = self
            .checkpoint
            .pools_checkpoint
            .lock()
            .unwrap()
            .block_number;
        let new_checkpoint_block = std::cmp::max(
            last_event_block.saturating_sub(MAX_REORG_BLOCK_COUNT),
            old_checkpoint_block,
        );

        if new_checkpoint_block > old_checkpoint_block {
            let block_range =
                RangeInclusive::try_new(old_checkpoint_block + 1, new_checkpoint_block)?;
            let events = self.events.lock().await.store().get_events(block_range);
            let mut checkpoint = self.checkpoint.pools_checkpoint.lock().unwrap();
            append_events(&mut checkpoint.pools, events);
            checkpoint.block_number = new_checkpoint_block;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl PoolFetching for UniswapV3PoolFetcher {
    async fn fetch(
        &self,
        token_pairs: &HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<PoolInfo>> {
        let block_number = match at_block {
            Block::Recent => self
                .events
                .lock()
                .await
                .store()
                .last_event_block()
                .await
                .unwrap_or_default(),
            Block::Number(number) => number,
        };

        // sanity check if events are up to date
        let last_handled_block = self
            .events
            .lock()
            .await
            .last_handled_block()
            .unwrap_or_default()
            .0;
        if block_number > last_handled_block {
            tracing::debug!(
                "can't get liquidity for block {} since the last handled block is {}",
                block_number,
                last_handled_block
            );
            return Ok(Default::default());
        }

        // this is the only place where this function uses checkpoint - no data racing between maintenance
        let (mut checkpoint, checkpoint_block_number) = self.checkpoint.get(token_pairs);

        if block_number > checkpoint_block_number {
            let block_range = RangeInclusive::try_new(checkpoint_block_number + 1, block_number)?;
            let events = self.events.lock().await.store().get_events(block_range);
            append_events(&mut checkpoint, events);
        }

        Ok(checkpoint
            .into_values()
            .flat_map(TryInto::try_into)
            .collect())
    }
}

/// For a given checkpoint, append events to get a new checkpoint
fn append_events(pools: &mut HashMap<H160, PoolData>, events: Vec<Event<UniswapV3Event>>) {
    for event in &events {
        let address = event
            .meta
            .as_ref()
            .expect("metadata must exist for mined blocks")
            .address;
        if let Some(pool) = pools.get_mut(&address) {
            match &event.data {
                UniswapV3Event::Burn(burn) => {
                    let tick_lower = BigInt::from(burn.tick_lower);
                    let tick_upper = BigInt::from(burn.tick_upper);

                    //liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if tick_lower <= pool.tick && pool.tick < tick_upper {
                        pool.liquidity -= burn.amount.into();
                    }

                    if let Some(ticks) = &mut pool.ticks {
                        //todo optimize to map
                        if ticks.iter().all(|tick| tick.tick_idx != tick_lower) {
                            ticks.push(TickData {
                                id: address.to_string() + "#" + &tick_lower.to_string(),
                                tick_idx: tick_lower.clone(),
                                liquidity_net: 0.into(),
                                pool_address: address,
                            });
                        }

                        if ticks.iter().all(|tick| tick.tick_idx != tick_upper) {
                            ticks.push(TickData {
                                id: address.to_string() + "#" + &tick_upper.to_string(),
                                tick_idx: tick_upper.clone(),
                                liquidity_net: 0.into(),
                                pool_address: address,
                            });
                        }

                        for tick in ticks {
                            if tick.tick_idx == tick_lower {
                                tick.liquidity_net -= BigInt::from(burn.amount);
                            }
                            if tick.tick_idx == tick_upper {
                                tick.liquidity_net += BigInt::from(burn.amount);
                            }
                        }
                    }
                }
                UniswapV3Event::Mint(mint) => {
                    let tick_lower = BigInt::from(mint.tick_lower);
                    let tick_upper = BigInt::from(mint.tick_upper);

                    //liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if tick_lower <= pool.tick && pool.tick < tick_upper {
                        pool.liquidity += mint.amount.into();
                    }

                    if let Some(ticks) = &mut pool.ticks {
                        //todo optimize to map
                        if ticks.iter().all(|tick| tick.tick_idx != tick_lower) {
                            ticks.push(TickData {
                                id: address.to_string() + "#" + &tick_lower.to_string(),
                                tick_idx: tick_lower.clone(),
                                liquidity_net: 0.into(),
                                pool_address: address,
                            });
                        }

                        if ticks.iter().all(|tick| tick.tick_idx != tick_upper) {
                            ticks.push(TickData {
                                id: address.to_string() + "#" + &tick_upper.to_string(),
                                tick_idx: tick_upper.clone(),
                                liquidity_net: 0.into(),
                                pool_address: address,
                            });
                        }

                        for tick in ticks {
                            if tick.tick_idx == tick_lower {
                                tick.liquidity_net += BigInt::from(mint.amount);
                            }
                            if tick.tick_idx == tick_upper {
                                tick.liquidity_net -= BigInt::from(mint.amount);
                            }
                        }
                    }
                }
                UniswapV3Event::Swap(swap) => {
                    pool.tick = BigInt::from(swap.tick);
                    pool.liquidity = swap.liquidity.into();
                    pool.sqrt_price = swap.sqrt_price_x96;
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Maintaining for UniswapV3PoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        let (result1, result2) = futures::join!(
            self.events.run_maintenance(),
            self.checkpoint.update_missing_pools()
        );
        result1?;
        result2?;
        self.move_checkpoint_to_future().await
    }
}

#[cfg(test)]
mod tests {
    use crate::transport;

    use super::*;
    use serde_json::json;
    use std::{ops::Sub, str::FromStr};

    #[test]
    fn encode_decode_pool_info() {
        let json = json!({
            "address": "0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28",
            "tokens": [
                {
                    "id": "0xbef81556ef066ec840a540595c8d12f516b6378f",
                    "symbol": "BCZ",
                    "decimals": "18",
                },
                {
                    "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
                    "symbol": "WETH",
                    "decimals": "18",
                }
            ],
            "state": {
                "sqrt_price": "792216481398733702759960397",
                "liquidity": "303015134493562686441",
                "tick": "-92110",
                "liquidity_net":
                    {
                        "-122070": "104713649338178916454" ,
                        "-77030": "1182024318125220460617" ,
                        "67260": "5812623076452005012674" ,
                    }
                ,
                "fee": "1/100",
            },
            "gas_stats": {
                "mean": "300000",
            }
        });

        let pool = PoolInfo {
            address: H160::from_str("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28").unwrap(),
            tokens: vec![
                Token {
                    id: H160::from_str("0xbef81556ef066ec840a540595c8d12f516b6378f").unwrap(),
                    symbol: "BCZ".to_string(),
                    decimals: 18,
                },
                Token {
                    id: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
                    symbol: "WETH".to_string(),
                    decimals: 18,
                },
            ],
            state: PoolState {
                sqrt_price: U256::from_dec_str("792216481398733702759960397").unwrap(),
                liquidity: U256::from_dec_str("303015134493562686441").unwrap(),
                tick: BigInt::from_str("-92110").unwrap(),
                liquidity_net: BTreeMap::from([
                    (
                        BigInt::from_str("-122070").unwrap(),
                        BigInt::from_str("104713649338178916454").unwrap(),
                    ),
                    (
                        BigInt::from_str("-77030").unwrap(),
                        BigInt::from_str("1182024318125220460617").unwrap(),
                    ),
                    (
                        BigInt::from_str("67260").unwrap(),
                        BigInt::from_str("5812623076452005012674").unwrap(),
                    ),
                ]),
                fee: Ratio::new(10_000u32, 1_000_000u32),
            },
            gas_stats: PoolStats {
                mean_gas: U256::from(300000),
            },
        };

        let serialized = serde_json::to_value(pool.clone()).unwrap();
        let deserialized = serde_json::from_value::<PoolInfo>(json.clone()).unwrap();

        assert_eq!(json, serialized);
        assert_eq!(pool, deserialized);
    }

    #[tokio::test]
    #[ignore]
    async fn uniswap_v3_pool_fetcher_constructor_test() {
        let transport = transport::create_env_test_transport();
        let web3 = Web3::new(transport);
        let fetcher = UniswapV3PoolFetcher::new(1, Client::new(), web3)
            .await
            .unwrap();

        assert!(!fetcher.checkpoint.pools_by_token_pair.is_empty());
        assert!(!fetcher
            .checkpoint
            .pools_checkpoint
            .lock()
            .unwrap()
            .pools
            .is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_test() {
        let transport = transport::create_env_test_transport();
        let web3 = Web3::new(transport);
        let fetcher = UniswapV3PoolFetcher::new(1, Client::new(), web3.clone())
            .await
            .unwrap();
        fetcher.run_maintenance().await.unwrap();
        let token_pairs = HashSet::from([
            TokenPair::new(
                H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
            )
            .unwrap(),
            TokenPair::new(
                H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
                H160::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7").unwrap(),
            )
            .unwrap(),
        ]);

        // get pools through the pool fetcher at the latest_block
        let latest_block = web3.eth().block_number().await.unwrap().as_u64().sub(5); //sub5 to avoid searching subgraph for still not indexed block
        let mut pools = fetcher
            .fetch(&token_pairs, Block::Number(latest_block))
            .await
            .unwrap();
        pools.sort_by(|a, b| a.address.cmp(&b.address));

        // get the same pools using direct call to subgraph
        let graph_api = UniV3SubgraphClient::for_chain(1, Client::new()).unwrap();
        let pool_ids = pools.iter().map(|pool| pool.address).collect::<Vec<_>>();

        // first get at the block in history
        let block_number = fetcher
            .checkpoint
            .pools_checkpoint
            .lock()
            .unwrap()
            .block_number;
        let pools_history = graph_api
            .get_pools_with_ticks_by_ids(&pool_ids, block_number)
            .await
            .unwrap();
        let mut pools_history = pools_history
            .into_iter()
            .flat_map(TryInto::try_into)
            .collect::<Vec<PoolInfo>>();
        pools_history.sort_by(|a, b| a.address.cmp(&b.address));

        // second get at the latest_block
        let pools2 = graph_api
            .get_pools_with_ticks_by_ids(&pool_ids, latest_block)
            .await
            .unwrap();
        let mut pools2 = pools2
            .into_iter()
            .flat_map(TryInto::try_into)
            .collect::<Vec<PoolInfo>>();
        pools2.sort_by(|a, b| a.address.cmp(&b.address));

        // observe results
        for pool in pools {
            dbg!("first address {} : {}", pool.address, pool.state);
        }
        for pool in pools2 {
            dbg!("second address {} : {}", pool.address, pool.state);
        }
        for pool in pools_history {
            dbg!("history address {} : {}", pool.address, pool.state);
        }
    }
}
