use crate::event_handling::EventHandler;
use crate::maintenance::Maintaining;
use crate::recent_block_cache::Block;
use crate::Web3;

use super::event_fetching::{RecentEventsCache, UniswapV3Event, UniswapV3PoolEventFetcher};
use super::graph_api::{PoolData, TickData, Token, UniV3SubgraphClient};
use anyhow::{Context, Result};
use ethcontract::{Event, H160, U256};
use itertools::{Either, Itertools};
use model::{u256_decimal, TokenPair};
use num::{rational::Ratio, BigInt, Zero};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: &HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<PoolInfo>>;
}

/// Pool data in a format prepared for solvers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PoolInfo {
    pub address: H160,
    pub tokens: Vec<Token>,
    pub state: PoolState,
    pub gas_stats: PoolStats,
}

/// Pool state in a format prepared for solvers.
#[serde_as]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PoolState {
    #[serde(with = "u256_decimal")]
    pub sqrt_price: U256,
    #[serde(with = "u256_decimal")]
    pub liquidity: U256,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub tick: BigInt,
    // (tick_idx, liquidity_net)
    #[serde_as(as = "HashMap<DisplayFromStr, DisplayFromStr>")]
    pub liquidity_net: HashMap<BigInt, BigInt>,
    #[serde(with = "serde_with::rust::display_fromstr")]
    pub fee: Ratio<u32>,
}

/// Pool stats in a format prepared for solvers
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    /// Pools state checkpoint on the `last_checkpoint_block` number.
    data: HashMap<H160, PoolData>,
    /// Last block considered reorg safe.
    last_checkpoint_block: Option<u64>,
}

pub struct UniswapV3PoolFetcher {
    web3: Web3,
    /// Graph api is used in two different situations:
    /// 1. once in constructor, to get the initial list of existing pools without their state.
    /// 2. once per each pool, to get the state, right at the moment when that pool state is requested by the user.
    /// This is done in order to avoid fetching all pools state at the same time for performance issues.
    graph_api: UniV3SubgraphClient,
    /// H160 is pool id while TokenPair is a pair or tokens for each pool.
    pools_by_token_pair: HashMap<TokenPair, HashSet<H160>>,
    /// Pools state on a specific block number in history considered reorg safe
    pools_checkpoint: Mutex<PoolsCheckpoint>,
    /// Recent events used on top of pools_checkpoint to get the `latest_block` pools state.
    events: tokio::sync::Mutex<EventHandler<Web3, UniswapV3PoolEventFetcher, RecentEventsCache>>,
}

impl UniswapV3PoolFetcher {
    /// Retrieves all registered pools on Uniswap V3 subgraph, but without `ticks`.
    /// Pools checkpoint is initially empty, but is supposed to be updated
    /// either on fetch or on periodic maintenance update.
    pub async fn new(chain_id: u64, client: Client, web3: Web3) -> Result<Self> {
        let graph_api = UniV3SubgraphClient::for_chain(chain_id, client)?;
        let registered_pools = graph_api.get_registered_pools().await?;
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

        Ok(Self {
            web3: web3.clone(),
            graph_api,
            pools_by_token_pair,
            pools_checkpoint: Default::default(),
            events: tokio::sync::Mutex::new(EventHandler::new(
                web3.clone(),
                UniswapV3PoolEventFetcher {
                    web3: web3.clone(),
                    contracts: registered_pools.pools.iter().map(|pool| pool.id).collect(),
                },
                RecentEventsCache::default(),
                Some(registered_pools.fetched_block_number),
            )),
        })
    }

    /// Fetches pool states of existing pools (in the pools checkpoint) and a list of missing pools.
    fn get_pools_checkpoint_state(
        &self,
        token_pairs: &HashSet<TokenPair>,
    ) -> (HashMap<H160, PoolData>, Vec<H160>) {
        tracing::debug!("UniswapV3PoolFetcher::get_cached_pools");
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        tracing::debug!("pool_ids: {:?}", pool_ids);

        match pool_ids.peek() {
            Some(_) => {
                let pools_checkpoint = self.pools_checkpoint.lock().unwrap();
                pool_ids.partition_map(|pool_id| match pools_checkpoint.data.get(pool_id) {
                    Some(entry) => {
                        tracing::debug!("returning pool: {:?}", pool_id);
                        Either::Left((entry.id, entry.clone()))
                    }
                    _ => {
                        tracing::debug!("missing pool: {:?}", pool_id);
                        Either::Right(pool_id)
                    }
                })
            }
            None => Default::default(),
        }
    }

    /// For the pools not found in the state checkpoint, get initial state of them via graph
    /// and store that initial state into checkpoint
    async fn get_initial_state_of_missing_pools_and_store_into_checkpoint(
        &self,
        pool_ids: &[H160],
    ) -> Result<Vec<PoolData>> {
        tracing::debug!("get_missing_pools");
        // release the lock as we dont want to hold it accross get_pools_with_ticks_by_ids await
        let last_checkpoint_block = self.pools_checkpoint.lock().unwrap().last_checkpoint_block;
        let block_number = match last_checkpoint_block {
            Some(number) => number,
            None => self.graph_api.get_safe_block().await?,
        };

        let pools = self
            .graph_api
            .get_pools_with_ticks_by_ids(pool_ids, block_number)
            .await?;

        tracing::debug!("pools len: {}", pools.len());
        let mut pools_checkpoint = self.pools_checkpoint.lock().unwrap();
        if pools_checkpoint.last_checkpoint_block.unwrap_or_default() > block_number {
            // here, it is possible that the pools_checkpoint changed after previously releasing the lock,
            // therefore, we should skip to update the checkpoint with obsolete values.
            // no particular harm is done, just no liquidity will be provided for the subset of the token pairs this time
            // we also have an option to try again until success, but don't want to further stall the fetch function
            return Ok(vec![]);
        }

        for pool in &pools {
            pools_checkpoint.data.insert(pool.id, pool.clone());
        }
        // no harm is done if pools_checkpoint.last_checkpoint_block is Some before assignment, just overwrite
        pools_checkpoint.last_checkpoint_block = Some(block_number);
        Ok(pools)
    }
}

#[async_trait::async_trait]
impl PoolFetching for UniswapV3PoolFetcher {
    async fn fetch(
        &self,
        token_pairs: &HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<PoolInfo>> {
        tracing::debug!("token_pairs {:?}", token_pairs);

        let block_number = match at_block {
            Block::Recent => self
                .web3
                .eth()
                .block_number()
                .await
                .expect("block_number")
                .as_u64(),
            Block::Number(number) => number,
        };

        let (mut existing_pools_checkpoint, missing_pools) =
            self.get_pools_checkpoint_state(token_pairs);
        tracing::debug!(
            "existing pools: {:?}, missing pools: {:?}",
            existing_pools_checkpoint,
            missing_pools
        );

        if !missing_pools.is_empty() {
            let missing_pools_checkpoint = self
                .get_initial_state_of_missing_pools_and_store_into_checkpoint(&missing_pools)
                .await?;
            tracing::debug!("missing pools checkpoint: {:?}", missing_pools_checkpoint);
            existing_pools_checkpoint.extend(
                missing_pools_checkpoint
                    .into_iter()
                    .map(|pool| (pool.id, pool)),
            );
        }

        let events_since_checkpoint = self
            .events
            .lock()
            .await
            .store()
            .get_events(block_number)
            .await?;

        append_events(&mut existing_pools_checkpoint, events_since_checkpoint);

        Ok(existing_pools_checkpoint
            .into_values()
            .flat_map(TryInto::try_into)
            .collect())
    }
}

/// For a given checkpoint, append events to get a new checkpoint
fn append_events(pools: &mut HashMap<H160, PoolData>, events: Vec<Event<UniswapV3Event>>) {
    for event in &events {
        let address = event.meta.as_ref().expect("metadata must exist for mined blocks").address;
        if let Some(pool) = pools.get_mut(&address) {
            match &event.data {
                UniswapV3Event::Burn(burn) => {
                    let tick_lower = BigInt::from(burn.tick_lower);
                    let tick_upper = BigInt::from(burn.tick_upper);

                    //liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if pool.tick <= tick_lower && pool.tick > tick_upper {
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
                    if pool.tick <= tick_lower && pool.tick > tick_upper {
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
        // self.events.run_maintenance().await.and_then(|()| {
        //     //update pools
        // })

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::transport;

    use super::*;
    use serde_json::json;
    use std::str::FromStr;

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
                liquidity_net: HashMap::from([
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

        assert!(!fetcher.pools_by_token_pair.is_empty());
        assert!(fetcher.pools_checkpoint.lock().unwrap().data.is_empty());
        assert!(fetcher
            .pools_checkpoint
            .lock()
            .unwrap()
            .last_checkpoint_block
            .is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_test() {
        let transport = transport::create_env_test_transport();
        let web3 = Web3::new(transport);
        let fetcher = UniswapV3PoolFetcher::new(1, Client::new(), web3.clone())
            .await
            .unwrap();
        let token_pairs = HashSet::from([TokenPair::new(
            H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
        )
        .unwrap()]);
        let block_number = Block::Number(web3.eth().block_number().await.unwrap().as_u64());
        let pools = fetcher.fetch(&token_pairs, block_number).await.unwrap();
        assert!(!pools.is_empty());
    }
}
