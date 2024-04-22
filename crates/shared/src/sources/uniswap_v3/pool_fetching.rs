use {
    super::{
        event_fetching::{RecentEventsCache, UniswapV3Event, UniswapV3PoolEventFetcher},
        graph_api::{PoolData, Token, UniV3SubgraphClient},
    },
    crate::{
        event_handling::{EventHandler, EventStoring, MAX_REORG_BLOCK_COUNT},
        maintenance::Maintaining,
        recent_block_cache::Block,
    },
    anyhow::{Context, Result},
    ethcontract::{Event, H160, U256},
    ethrpc::{
        current_block::{BlockRetrieving, RangeInclusive},
        Web3,
    },
    itertools::{Either, Itertools},
    model::TokenPair,
    num::{rational::Ratio, BigInt, Zero},
    number::serialization::HexOrDecimalU256,
    reqwest::{Client, Url},
    serde::Serialize,
    serde_with::{serde_as, DisplayFromStr},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        ops::Neg,
        sync::{Arc, Mutex},
    },
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
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PoolInfo {
    /// Skip serializing address since it's redundant (already serialized
    /// outside of this struct)
    #[serde(skip_serializing)]
    pub address: H160,
    pub tokens: Vec<Token>,
    pub state: PoolState,
    pub gas_stats: PoolStats,
}

/// Pool state in a format prepared for solvers.
#[serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PoolState {
    #[serde_as(as = "HexOrDecimalU256")]
    pub sqrt_price: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    pub liquidity: U256,
    #[serde_as(as = "DisplayFromStr")]
    pub tick: BigInt,
    // (tick_idx, liquidity_net)
    #[serde_as(as = "BTreeMap<DisplayFromStr, DisplayFromStr>")]
    pub liquidity_net: BTreeMap<BigInt, BigInt>,
    #[serde(skip_serializing)]
    pub fee: Ratio<u32>,
}

/// Pool stats in a format prepared for solvers
#[serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PoolStats {
    #[serde_as(as = "HexOrDecimalU256")]
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
                mean_gas: U256::from(108_163), // as estimated by https://dune.com/queries/1044812
            },
        })
    }
}

#[derive(Default)]
struct PoolsCheckpoint {
    /// Pools state.
    pools: HashMap<H160, PoolInfo>,
    /// Block number for which `pools` field was populated.
    block_number: u64,
    /// Pools that don't exist in `pools` field, therefore need to be
    /// initialized and moved to `pools` in the next maintainance run
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
    /// Fetches the list of existing UniswapV3 pools and their metadata (without
    /// state/ticks). Then fetches state/ticks for the most deepest pools
    /// (subset of all existing pools)
    pub async fn new(
        subgraph_url: &Url,
        client: Client,
        max_pools_to_initialize_cache: usize,
    ) -> Result<Self> {
        let graph_api = UniV3SubgraphClient::from_subgraph_url(subgraph_url, client)?;
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
        // so let's fetch the top `max_pools_to_initialize_cache` pools with the highest
        // liquidity
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
            .take(max_pools_to_initialize_cache)
            .collect::<Vec<_>>();
        let pools = graph_api
            .get_pools_with_ticks_by_ids(&pool_ids, registered_pools.fetched_block_number)
            .await?
            .into_iter()
            .filter_map(|pool| Some((pool.id, pool.try_into().ok()?)))
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

    /// For a given list of token pairs, fetches the pools for the ones that
    /// exist in the checkpoint. For the ones that don't exist, flag as
    /// missing and expect to exist after the next maintenance run.
    fn get(&self, token_pairs: &HashSet<TokenPair>) -> (HashMap<H160, PoolInfo>, u64) {
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        tracing::trace!("get checkpoint for pool_ids: {:?}", pool_ids);

        match pool_ids.peek() {
            Some(_) => {
                let mut pools_checkpoint = self.pools_checkpoint.lock().unwrap();
                let (existing_pools, missing_pools): (HashMap<H160, PoolInfo>, Vec<H160>) =
                    pool_ids.partition_map(|pool_id| match pools_checkpoint.pools.get(pool_id) {
                        Some(entry) => Either::Left((*pool_id, entry.clone())),
                        _ => Either::Right(pool_id),
                    });
                tracing::trace!(
                    "cache hit: {:?}, cache miss: {:?}",
                    existing_pools.keys(),
                    missing_pools
                );
                pools_checkpoint.missing_pools.extend(missing_pools);
                (existing_pools, pools_checkpoint.block_number)
            }
            None => Default::default(),
        }
    }

    /// Fetches state/ticks for missing pools and moves them from
    /// `missing_pools` to `pools`
    async fn update_missing_pools(&self) -> Result<()> {
        let (missing_pools, block_number) = {
            let checkpoint = self.pools_checkpoint.lock().unwrap();
            (checkpoint.missing_pools.clone(), checkpoint.block_number)
        };
        tracing::debug!("currently missing pools are {:?}", missing_pools);

        let pool_ids = missing_pools.into_iter().collect::<Vec<_>>();
        let start = std::time::Instant::now();
        let pools = self
            .graph_api
            .get_pools_with_ticks_by_ids(&pool_ids, block_number)
            .await;
        tracing::debug!(
            requested_pools = pool_ids.len(),
            time = ?start.elapsed(),
            request_successful = pools.is_ok(),
            "fetched pool ticks"
        );

        let mut checkpoint = self.pools_checkpoint.lock().unwrap();
        for pool in pools? {
            checkpoint.missing_pools.remove(&pool.id);
            checkpoint.pools.insert(pool.id, pool.try_into()?);
        }

        tracing::debug!("number of cached pools is {}", checkpoint.pools.len());
        if !checkpoint.missing_pools.is_empty() {
            tracing::warn!(
                "not all missing pools updated: {:?}",
                checkpoint.missing_pools
            );
        }
        Ok(())
    }
}

pub struct UniswapV3PoolFetcher {
    /// Pools state on a specific block number in history considered reorg safe
    checkpoint: PoolsCheckpointHandler,
    /// Recent events used on top of pools_checkpoint to get the `latest_block`
    /// pools state.
    events: tokio::sync::Mutex<EventHandler<UniswapV3PoolEventFetcher, RecentEventsCache>>,
}

impl UniswapV3PoolFetcher {
    pub async fn new(
        subgraph_url: &Url,
        web3: Web3,
        client: Client,
        block_retriever: Arc<dyn BlockRetrieving>,
        max_pools_to_initialize: usize,
    ) -> Result<Self> {
        let web3 = ethrpc::instrumented::instrument_with_label(&web3, "uniswapV3".into());
        let checkpoint =
            PoolsCheckpointHandler::new(subgraph_url, client, max_pools_to_initialize).await?;

        let init_block = checkpoint.pools_checkpoint.lock().unwrap().block_number;
        let init_block = block_retriever.block(init_block).await?;

        let events = tokio::sync::Mutex::new(EventHandler::new(
            block_retriever,
            UniswapV3PoolEventFetcher(web3),
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
            {
                let block_range =
                    RangeInclusive::try_new(old_checkpoint_block + 1, new_checkpoint_block)?;
                let events = self.events.lock().await.store().get_events(block_range);
                let mut checkpoint = self.checkpoint.pools_checkpoint.lock().unwrap();
                append_events(&mut checkpoint.pools, events);
                checkpoint.block_number = new_checkpoint_block;
                tracing::debug!(
                    "checkpoint block number updated to {}",
                    checkpoint.block_number
                );
            }

            // clear events with block number lower than `new_checkpoint_block`
            self.events
                .lock()
                .await
                .store_mut()
                .remove_events_older_than_block(new_checkpoint_block);
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
            // we call run_maintenance() here because that is the only way to update the
            // event storage with the events from the block range
            // last_handled_block..=block_number which are missing
            if let Err(err) = self.events.run_maintenance().await {
                tracing::debug!("failed to update events on fetch because {}", err);
                return Ok(Default::default());
            }
        }

        // this is the only place where this function uses checkpoint - no data racing
        // between maintenance
        let (mut checkpoint, checkpoint_block_number) = self.checkpoint.get(token_pairs);

        if block_number > checkpoint_block_number {
            let block_range = RangeInclusive::try_new(checkpoint_block_number + 1, block_number)?;
            let events = self.events.lock().await.store().get_events(block_range);
            append_events(&mut checkpoint, events);
        }

        // return only pools which current liquidity is positive
        Ok(checkpoint
            .into_values()
            .filter(|pool| pool.state.liquidity > U256::zero())
            .collect())
    }
}

/// For a given checkpoint, append events to get a new checkpoint
fn append_events(pools: &mut HashMap<H160, PoolInfo>, events: Vec<Event<UniswapV3Event>>) {
    for event in events {
        let address = event
            .meta
            .expect("metadata must exist for mined blocks")
            .address;
        if let Some(pool) = pools.get_mut(&address).map(|pool| &mut pool.state) {
            match event.data {
                UniswapV3Event::Burn(burn) => {
                    let tick_lower = BigInt::from(burn.tick_lower);
                    let tick_upper = BigInt::from(burn.tick_upper);

                    // liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if tick_lower <= pool.tick && pool.tick < tick_upper {
                        pool.liquidity -= burn.amount.into();
                    }

                    pool.liquidity_net
                        .entry(tick_lower.clone())
                        .and_modify(|tick| *tick -= BigInt::from(burn.amount))
                        .or_insert_with(|| BigInt::from(burn.amount).neg());

                    pool.liquidity_net
                        .entry(tick_upper.clone())
                        .and_modify(|tick| *tick += BigInt::from(burn.amount))
                        .or_insert_with(|| BigInt::from(burn.amount));

                    // remove 0 entries to save bandwidth
                    if pool.liquidity_net[&tick_lower].is_zero() {
                        pool.liquidity_net.remove(&tick_lower);
                    }

                    if pool.liquidity_net[&tick_upper].is_zero() {
                        pool.liquidity_net.remove(&tick_upper);
                    }
                }
                UniswapV3Event::Mint(mint) => {
                    let tick_lower = BigInt::from(mint.tick_lower);
                    let tick_upper = BigInt::from(mint.tick_upper);

                    // liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if tick_lower <= pool.tick && pool.tick < tick_upper {
                        pool.liquidity += mint.amount.into();
                    }

                    pool.liquidity_net
                        .entry(tick_lower.clone())
                        .and_modify(|tick| *tick += BigInt::from(mint.amount))
                        .or_insert_with(|| BigInt::from(mint.amount));

                    pool.liquidity_net
                        .entry(tick_upper.clone())
                        .and_modify(|tick| *tick -= BigInt::from(mint.amount))
                        .or_insert_with(|| BigInt::from(mint.amount).neg());

                    // remove 0 entries to save bandwidth
                    if pool.liquidity_net[&tick_lower].is_zero() {
                        pool.liquidity_net.remove(&tick_lower);
                    }

                    if pool.liquidity_net[&tick_upper].is_zero() {
                        pool.liquidity_net.remove(&tick_upper);
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
        // since failure in updating the missing pools is not critical for
        // UniswapV3PoolFetcher maintenance and future liquidity fetch calls,
        // then there is no need to return error
        if let Err(err) = result2 {
            tracing::warn!(
                "UniswapV3PoolFetcher failed to update missing pools: {}",
                err
            );
        }
        self.move_checkpoint_to_future().await
    }

    fn name(&self) -> &str {
        "UniswapV3PoolFetcher"
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        contracts::uniswap_v3_pool::event_data::{Burn, Mint, Swap},
        ethcontract::EventMetadata,
        serde_json::json,
        std::str::FromStr,
    };

    #[test]
    fn encode_decode_pool_info() {
        let json = json!({
            "tokens": [
                {
                    "id": "0xbef81556ef066ec840a540595c8d12f516b6378f",
                    "decimals": "18",
                },
                {
                    "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
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
                    decimals: 18,
                },
                Token {
                    id: H160::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
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

        let serialized = serde_json::to_value(pool).unwrap();
        assert_eq!(json, serialized);
    }

    #[test]
    fn append_events_test_empty() {
        let pools = HashMap::from([(H160::from_low_u64_be(1), Default::default())]);
        let mut new_pools = pools.clone();
        let events = vec![];
        append_events(&mut new_pools, events);
        assert_eq!(new_pools, pools);
    }

    #[test]
    fn append_events_test_swap() {
        let address = H160::from_low_u64_be(1);
        let pool = PoolInfo {
            address,
            ..Default::default()
        };
        let mut pools = HashMap::from([(address, pool)]);

        let event = Event {
            data: UniswapV3Event::Swap(Swap {
                sqrt_price_x96: 1.into(),
                liquidity: 2,
                tick: 3,
                ..Default::default()
            }),
            meta: Some(EventMetadata {
                address,
                ..Default::default()
            }),
        };
        append_events(&mut pools, vec![event]);

        assert_eq!(pools[&address].state.tick, BigInt::from(3));
        assert_eq!(pools[&address].state.liquidity, U256::from(2));
        assert_eq!(pools[&address].state.sqrt_price, U256::from(1));
    }

    #[test]
    fn append_events_test_burn() {
        let address = H160::from_low_u64_be(1);
        let pool = PoolInfo {
            address,
            ..Default::default()
        };
        let mut pools = HashMap::from([(address, pool)]);

        // add first burn event
        let event = Event {
            data: UniswapV3Event::Burn(Burn {
                tick_lower: 100000,
                tick_upper: 110000,
                amount: 12345,
                ..Default::default()
            }),
            meta: Some(EventMetadata {
                address,
                ..Default::default()
            }),
        };
        append_events(&mut pools, vec![event]);
        assert_eq!(
            pools[&address].state.liquidity_net,
            BTreeMap::from([
                (BigInt::from(100_000), BigInt::from(-12345)),
                (BigInt::from(110_000), BigInt::from(12345))
            ])
        );

        // add second burn event
        let event = Event {
            data: UniswapV3Event::Burn(Burn {
                tick_lower: 105000,
                tick_upper: 110000,
                amount: 54321,
                ..Default::default()
            }),
            meta: Some(EventMetadata {
                address,
                ..Default::default()
            }),
        };
        append_events(&mut pools, vec![event]);
        assert_eq!(
            pools[&address].state.liquidity_net,
            BTreeMap::from([
                (BigInt::from(100_000), BigInt::from(-12345)),
                (BigInt::from(105_000), BigInt::from(-54321)),
                (BigInt::from(110_000), BigInt::from(66666))
            ])
        );
    }

    #[test]
    fn append_events_test_mint() {
        let address = H160::from_low_u64_be(1);
        let pool = PoolInfo {
            address,
            ..Default::default()
        };
        let mut pools = HashMap::from([(address, pool)]);

        // add first mint event
        let event = Event {
            data: UniswapV3Event::Mint(Mint {
                tick_lower: 100000,
                tick_upper: 110000,
                amount: 12345,
                ..Default::default()
            }),
            meta: Some(EventMetadata {
                address,
                ..Default::default()
            }),
        };
        append_events(&mut pools, vec![event]);
        assert_eq!(
            pools[&address].state.liquidity_net,
            BTreeMap::from([
                (BigInt::from(100_000), BigInt::from(12345)),
                (BigInt::from(110_000), BigInt::from(-12345))
            ])
        );

        // add second burn event
        let event = Event {
            data: UniswapV3Event::Mint(Mint {
                tick_lower: 105000,
                tick_upper: 110000,
                amount: 54321,
                ..Default::default()
            }),
            meta: Some(EventMetadata {
                address,
                ..Default::default()
            }),
        };
        append_events(&mut pools, vec![event]);
        assert_eq!(
            pools[&address].state.liquidity_net,
            BTreeMap::from([
                (BigInt::from(100_000), BigInt::from(12345)),
                (BigInt::from(105_000), BigInt::from(54321)),
                (BigInt::from(110_000), BigInt::from(-66666))
            ])
        );
    }
}
