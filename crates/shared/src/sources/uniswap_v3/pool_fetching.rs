use crate::maintenance::Maintaining;

use super::graph_api::{PoolData, Token, UniV3SubgraphClient};
use anyhow::{Context, Result};
use ethcontract::{H160, U256};
use itertools::{Either, Itertools};
use model::u256_decimal;
use model::TokenPair;
use num::{rational::Ratio, BigInt, Zero};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(&self, token_pairs: &HashSet<TokenPair>) -> Result<Vec<PoolInfo>>;
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
    #[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")]
    pub liquidity_net: Vec<(BigInt, BigInt)>,
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
            tokens: vec![
                pool.token0.context("no token0")?,
                pool.token1.context("no token1")?,
            ],
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
                fee: Ratio::new(pool.fee_tier.context("no fee")?.as_u32(), 1_000_000u32),
            },
            gas_stats: PoolStats {
                mean_gas: U256::from(300_000), // todo: hardcoded for testing purposes
            },
        })
    }
}

pub struct CachedPool {
    pub pool: PoolData,
    pub updated_at: Instant,
    pub requested_at: Instant,
}

pub struct UniswapV3PoolFetcher {
    graph_api: UniV3SubgraphClient,
    /// H160 is pool id while TokenPair is a pair or tokens for each pool
    pools_by_token_pair: HashMap<TokenPair, HashSet<H160>>,
    cache: Mutex<HashMap<H160, CachedPool>>,
    max_age: Duration,
}

impl UniswapV3PoolFetcher {
    /// Retrieves all registered pools on Uniswap V3 subgraph, but without `ticks`,
    /// making the cache values outdated immediately. Cache values are supposed to be updated
    /// either on fetch or on periodic maintenance update.
    pub async fn new(chain_id: u64, max_age: Duration, client: Client) -> Result<Self> {
        let graph_api = UniV3SubgraphClient::for_chain(chain_id, client)?;
        let registered_pools = graph_api.get_registered_pools().await?;
        tracing::debug!(
            block = %registered_pools.fetched_block_number, pools = %registered_pools.pools.len(),
            "initialized registered pools",
        );

        let mut pools_by_token_pair: HashMap<TokenPair, HashSet<H160>> = HashMap::new();
        for pool in registered_pools.pools {
            let token0 = pool.token0.clone().context("token0 does not exist")?.id;
            let token1 = pool.token1.clone().context("token1 does not exist")?.id;

            let pair = TokenPair::new(token0, token1).context("cant create pair")?;
            pools_by_token_pair.entry(pair).or_default().insert(pool.id);
        }

        Ok(Self {
            pools_by_token_pair,
            graph_api,
            cache: Default::default(),
            max_age,
        })
    }

    async fn get_pools_and_update_cache(&self, pool_ids: &[H160]) -> Result<Vec<PoolData>> {
        tracing::debug!("get_pools_and_update_cache");
        let pools = self.graph_api.get_pools_with_ticks_by_ids(pool_ids).await?;
        tracing::debug!("pools len: {}", pools.len());
        let now = Instant::now();
        let mut cache = self.cache.lock().unwrap();
        for pool in &pools {
            cache.insert(
                pool.id,
                CachedPool {
                    pool: pool.clone(),
                    updated_at: now,
                    requested_at: now,
                },
            );
        }
        Ok(pools)
    }

    /// Returns cached pools and ids of outdated pools.
    fn get_cached_pools(&self, token_pairs: &HashSet<TokenPair>) -> (Vec<PoolData>, Vec<H160>) {
        tracing::debug!("UniswapV3PoolFetcher::get_cached_pools");
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        tracing::debug!("pool_ids: {:?}", pool_ids);

        match pool_ids.peek() {
            Some(_) => {
                let now = Instant::now();
                let mut cache = self.cache.lock().unwrap();
                pool_ids.partition_map(|pool_id| match cache.get_mut(pool_id) {
                    Some(entry)
                        if now.saturating_duration_since(entry.updated_at) < self.max_age =>
                    {
                        tracing::debug!("returning cached pool: {:?}", pool_id);
                        entry.requested_at = now;
                        Either::Left(entry.pool.clone())
                    }
                    _ => {
                        tracing::debug!("returning outdated pool: {:?}", pool_id);
                        Either::Right(pool_id)
                    }
                })
            }
            None => Default::default(),
        }
    }
}

#[async_trait::async_trait]
impl PoolFetching for UniswapV3PoolFetcher {
    async fn fetch(&self, token_pairs: &HashSet<TokenPair>) -> Result<Vec<PoolInfo>> {
        tracing::debug!("token_pairs {:?}", token_pairs);
        let (mut cached_pools, outdated_pools) = self.get_cached_pools(token_pairs);
        tracing::debug!(
            "cached pools: {:?}, outdated pools: {:?}",
            cached_pools,
            outdated_pools
        );

        if !outdated_pools.is_empty() {
            let updated_pools = self.get_pools_and_update_cache(&outdated_pools).await?;
            tracing::debug!("updated pools: {:?}", updated_pools);
            cached_pools.extend(updated_pools);
        }

        Ok(cached_pools
            .into_iter()
            .flat_map(TryInto::try_into)
            .collect())
    }
}

#[async_trait::async_trait]
impl Maintaining for UniswapV3PoolFetcher {
    async fn run_maintenance(&self) -> Result<()> {
        tracing::debug!("periodic update started");
        let now = Instant::now();

        let mut outdated_entries = self
            .cache
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, cached)| now.saturating_duration_since(cached.updated_at) > self.max_age)
            .map(|(pool_id, cached)| (*pool_id, cached.requested_at))
            .collect::<Vec<_>>();
        outdated_entries.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        tracing::debug!("outdated_entries {:?}", outdated_entries);

        let pools_to_update = outdated_entries
            .iter()
            .map(|(pool_id, _)| *pool_id)
            .collect::<Vec<_>>();
        tracing::debug!("pools to update {:?}", pools_to_update);

        if !pools_to_update.is_empty() {
            if let Err(err) = self.get_pools_and_update_cache(&pools_to_update).await {
                tracing::warn!(
                    error = %err,
                    "failed to update pools",
                );
            }
        }

        tracing::debug!("periodic update ended");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
                liquidity_net: vec![
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
                ],
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
    async fn uniswap_v3_pool_fetcher_test() {
        let fetcher = UniswapV3PoolFetcher::new(1, Duration::from_secs(10), Client::new())
            .await
            .unwrap();

        assert!(!fetcher.pools_by_token_pair.is_empty());
        assert!(!fetcher.cache.lock().unwrap().is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_test() {
        let fetcher = UniswapV3PoolFetcher::new(1, Duration::from_secs(10), Client::new())
            .await
            .unwrap();
        let token_pairs = HashSet::from([TokenPair::new(
            H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap(),
            H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap(),
        )
        .unwrap()]);
        let pools = fetcher.fetch(&token_pairs).await.unwrap();
        assert!(!pools.is_empty());
    }
}
