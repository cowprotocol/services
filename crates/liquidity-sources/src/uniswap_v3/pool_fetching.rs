use {
    super::{
        V3PoolDataSource,
        event_fetching::{RecentEventsCache, UniswapV3PoolEventFetcher},
        graph_api::{PoolData, Token},
    },
    crate::{recent_block_cache::Block, uniswap_v3::event_fetching::WithAddress},
    alloy::{
        primitives::{Address, U256},
        rpc::types::Log,
    },
    anyhow::{Context, Result},
    contracts::UniswapV3Pool::UniswapV3Pool::{
        UniswapV3PoolEvents as AlloyUniswapV3PoolEvents,
        UniswapV3PoolEvents,
    },
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    event_indexing::{
        block_retriever::{BlockRetrieving, RangeInclusive},
        event_handler::{EventHandler, EventStoring, MAX_REORG_BLOCK_COUNT},
        maintenance::Maintaining,
    },
    model::TokenPair,
    num::rational::Ratio,
    number::serialization::HexOrDecimalU256,
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        sync::{Arc, Mutex},
    },
    tracing::instrument,
};

#[async_trait::async_trait]
pub trait PoolFetching: Send + Sync {
    async fn fetch(
        &self,
        token_pairs: &HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Arc<PoolInfo>>>;
}

/// Pool data in a format prepared for solvers.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct PoolInfo {
    /// Skip serializing address since it's redundant (already serialized
    /// outside of this struct)
    #[serde(skip_serializing)]
    pub address: Address,
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
    pub tick: i32,
    // (tick_idx, liquidity_net). `Arc`-shared so cloning a `PoolInfo` doesn't
    // deep-copy the tick map; copied-on-write only when mutated (Mint/Burn).
    #[serde_as(as = "Arc<BTreeMap<DisplayFromStr, DisplayFromStr>>")]
    pub liquidity_net: Arc<BTreeMap<i32, i128>>,
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
                liquidity_net: Arc::new(
                    pool.ticks
                        .context("no ticks")?
                        .into_iter()
                        .filter_map(|tick| {
                            if tick.liquidity_net == 0 {
                                None
                            } else {
                                Some((tick.tick_idx, tick.liquidity_net))
                            }
                        })
                        .collect(),
                ),
                fee: Ratio::new(u32::try_from(pool.fee_tier)?, 1_000_000u32),
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
    pools: HashMap<Address, Arc<PoolInfo>>,
    /// Block number for which `pools` field was populated.
    block_number: u64,
    /// Pools that don't exist in `pools` field, therefore need to be
    /// initialized and moved to `pools` in the next maintainance run
    missing_pools: HashSet<Address>,
}

struct PoolsCheckpointHandler {
    source: Arc<dyn V3PoolDataSource>,
    /// Address is pool id while TokenPair is a pair or tokens for each pool.
    pools_by_token_pair: HashMap<TokenPair, Vec<Address>>,
    /// Pools state on a specific block number in history considered reorg safe
    pools_checkpoint: Mutex<PoolsCheckpoint>,
}

impl PoolsCheckpointHandler {
    /// Fetches the list of existing UniswapV3 pools and their metadata (without
    /// state/ticks). Then fetches state/ticks for the deepest pools
    /// (subset of all existing pools).
    ///
    /// `target_block` is the chain's finalized block so it matches the
    /// pool-indexer source's anchor; both calls then return data at or
    /// after that block. The event-replay anchor is taken from the *tick*
    /// call's response (the later of the two). Otherwise there's a race
    /// between the pool-list fetch and the tick fetch: an event landing
    /// between them would show up in `ticks` but not `pools`, and replaying
    /// it would apply that event twice.
    pub async fn new(
        source: Arc<dyn V3PoolDataSource>,
        block_retriever: Arc<dyn BlockRetrieving>,
        max_pools_to_initialize_cache: usize,
    ) -> Result<Self> {
        let target_block = block_retriever
            .finalized_block()
            .await
            .context("read finalized block for snapshot target_block")?
            .number;
        let mut registered_pools = source.get_registered_pools(target_block).await?;
        tracing::debug!(
            target_block,
            block = %registered_pools.fetched_block_number,
            pools = %registered_pools.pools.len(),
            "initialized registered pools",
        );

        let pools_by_token_pair = {
            // we store addresses in a `Vec` instead of a `HashSet` to save on memory but
            // we still ensure there are no duplicated pools.
            let mut pools_by_token_pair: HashMap<TokenPair, Vec<Address>> = HashMap::new();
            for pool in &registered_pools.pools {
                let pair =
                    TokenPair::new(pool.token0.id, pool.token1.id).context("cant create pair")?;
                let pools = pools_by_token_pair.entry(pair).or_default();
                if !pools.contains(&pool.id) {
                    pools.push(pool.id);
                }
            }
            pools_by_token_pair
                .values_mut()
                .for_each(|bucket| bucket.shrink_to_fit());
            pools_by_token_pair.shrink_to_fit();
            pools_by_token_pair
        };

        // can't fetch the state of all pools in constructor for performance reasons,
        // so let's fetch the top `max_pools_to_initialize_cache` pools with the highest
        // liquidity
        registered_pools
            .pools
            .sort_unstable_by(|a, b| a.liquidity.partial_cmp(&b.liquidity).unwrap());
        let pool_ids = registered_pools
            .pools
            .clone()
            .into_iter()
            .map(|pool| pool.id)
            .rev()
            .take(max_pools_to_initialize_cache)
            .collect::<Vec<_>>();
        let pools_with_ticks = source
            .get_pools_with_ticks_by_ids(&pool_ids, registered_pools.fetched_block_number)
            .await?;
        let pools = pools_with_ticks
            .pools
            .into_iter()
            .filter_map(|pool| Some((pool.id, Arc::new(pool.try_into().ok()?))))
            .collect::<HashMap<_, _>>();
        // Anchor the checkpoint at the *tick* call's snapshot block, which is
        // `>= registered_pools.fetched_block_number`. For pool-indexer-backed
        // sources this is later than the `get_registered_pools` call; using it
        // (not the earlier block) prevents the driver's event replay from
        // double-applying Mint/Burn events that the indexer already reflected
        // by the time the tick fetch returned.
        let pools_checkpoint = Mutex::new(PoolsCheckpoint {
            pools,
            block_number: pools_with_ticks.fetched_block_number,
            ..Default::default()
        });

        Ok(Self {
            source,
            pools_by_token_pair,
            pools_checkpoint,
        })
    }

    /// For a given list of token pairs, fetches the pools for the ones that
    /// exist in the checkpoint. For the ones that don't exist, flag as
    /// missing and expect to exist after the next maintenance run.
    fn get(&self, token_pairs: &HashSet<TokenPair>) -> (HashMap<Address, Arc<PoolInfo>>, u64) {
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        tracing::trace!("get checkpoint for pool_ids: {:?}", pool_ids);

        match pool_ids.peek() {
            Some(_) => {
                let mut pools_checkpoint = self.pools_checkpoint.lock().unwrap();
                let mut existing_pools = HashMap::<Address, Arc<PoolInfo>>::default();
                let missing_pools = pool_ids
                    .filter(|pool_id| match pools_checkpoint.pools.get(*pool_id) {
                        Some(entry) => {
                            existing_pools.insert(**pool_id, entry.clone());
                            false
                        }
                        None => true,
                    })
                    .collect::<Vec<_>>();

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
            if checkpoint.missing_pools.is_empty() {
                return Ok(());
            }
            (checkpoint.missing_pools.clone(), checkpoint.block_number)
        };
        tracing::debug!("currently missing pools are {:?}", missing_pools);

        let pool_ids = missing_pools.into_iter().collect::<Vec<_>>();
        let start = std::time::Instant::now();
        let pools_with_ticks = self
            .source
            .get_pools_with_ticks_by_ids(&pool_ids, block_number)
            .await;
        tracing::debug!(
            requested_pools = pool_ids.len(),
            time = ?start.elapsed(),
            request_successful = pools_with_ticks.is_ok(),
            "fetched pool ticks"
        );
        let pools_with_ticks = pools_with_ticks?;

        let mut checkpoint = self.pools_checkpoint.lock().unwrap();
        for pool in pools_with_ticks.pools {
            checkpoint.missing_pools.remove(&pool.id);
            checkpoint.pools.insert(pool.id, Arc::new(pool.try_into()?));
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
    events: tokio::sync::Mutex<
        EventHandler<UniswapV3PoolEventFetcher, RecentEventsCache, (AlloyUniswapV3PoolEvents, Log)>,
    >,
}

impl UniswapV3PoolFetcher {
    pub async fn new(
        source: Arc<dyn V3PoolDataSource>,
        web3: Web3,
        block_retriever: Arc<dyn BlockRetrieving>,
        max_pools_to_initialize: usize,
    ) -> Result<Self> {
        let web3 = web3.labeled("uniswapV3");
        let checkpoint =
            PoolsCheckpointHandler::new(source, block_retriever.clone(), max_pools_to_initialize)
                .await?;

        let init_block = checkpoint.pools_checkpoint.lock().unwrap().block_number;
        let init_block = block_retriever.block(init_block).await?;

        let events = tokio::sync::Mutex::new(EventHandler::new(
            block_retriever,
            UniswapV3PoolEventFetcher(web3.provider),
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
    #[instrument(skip_all)]
    async fn fetch(
        &self,
        token_pairs: &HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Arc<PoolInfo>>> {
        let block_number = match at_block {
            Block::Recent | Block::Finalized => self
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
            .filter(|pool| pool.state.liquidity > U256::ZERO)
            .collect())
    }
}

/// For a given checkpoint, append events to get a new checkpoint
fn append_events(
    pools: &mut HashMap<Address, Arc<PoolInfo>>,
    events: Vec<WithAddress<UniswapV3PoolEvents>>,
) {
    for event in events {
        if let Some(pool) = pools.get_mut(&event.address()).map(Arc::make_mut) {
            let pool = &mut pool.state;
            match event.inner() {
                UniswapV3PoolEvents::Burn(burn) => {
                    let tick_lower = burn.tickLower.as_i32();
                    let tick_upper = burn.tickUpper.as_i32();
                    // `amount` is the position's `uint128` liquidity and always fits
                    // `i128`: it's capped on-chain by `maxLiquidityPerTick` (~1.9e32) which
                    // is far below `i128::MAX` (~1.7e38), so this branch is unreachable for
                    // any valid event. We skip the whole event (not just the tick deltas)
                    // so `liquidity` and `liquidity_net` can't desync.
                    let Ok(amount) = i128::try_from(burn.amount) else {
                        tracing::warn!(amount = %burn.amount, "burn liquidity exceeds i128; skipping event");
                        continue;
                    };

                    // liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if tick_lower <= pool.tick && pool.tick < tick_upper {
                        pool.liquidity -= U256::from(burn.amount);
                    }

                    let liquidity_net = Arc::make_mut(&mut pool.liquidity_net);
                    update_liquidity_net(liquidity_net, tick_lower, -amount);
                    update_liquidity_net(liquidity_net, tick_upper, amount);
                }
                UniswapV3PoolEvents::Mint(mint) => {
                    let tick_lower = mint.tickLower.as_i32();
                    let tick_upper = mint.tickUpper.as_i32();
                    // Unreachable for the same reason as the `Burn` arm (per-position
                    // liquidity is capped well below `i128::MAX`); skip the whole event to
                    // avoid desyncing `liquidity` from `liquidity_net`.
                    let Ok(amount) = i128::try_from(mint.amount) else {
                        tracing::warn!(amount = %mint.amount, "mint liquidity exceeds i128; skipping event");
                        continue;
                    };

                    // liquidity tracks the liquidity on recent tick,
                    // only need to update it if the new position includes the recent tick.
                    if tick_lower <= pool.tick && pool.tick < tick_upper {
                        pool.liquidity += U256::from(mint.amount);
                    }

                    let liquidity_net = Arc::make_mut(&mut pool.liquidity_net);
                    update_liquidity_net(liquidity_net, tick_lower, amount);
                    update_liquidity_net(liquidity_net, tick_upper, -amount);
                }
                UniswapV3PoolEvents::Swap(swap) => {
                    pool.tick = swap.tick.as_i32();
                    pool.liquidity = U256::from(swap.liquidity);
                    pool.sqrt_price = U256::from(swap.sqrtPriceX96);
                }
                _ => continue,
            }
        }
    }
}

/// Applies a signed `delta` to a tick's net liquidity, dropping the entry when
/// it cancels out to zero. The accumulated value mirrors the pool's on-chain
/// `int128` `liquidityNet`, so a real overflow is impossible; `saturating_add`
/// just keeps us panic-free against malformed event data.
fn update_liquidity_net(liquidity_net: &mut BTreeMap<i32, i128>, tick: i32, delta: i128) {
    let entry = liquidity_net.entry(tick).or_insert(0);
    *entry = entry.saturating_add(delta);
    if *entry == 0 {
        liquidity_net.remove(&tick);
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
        alloy::primitives::{U160, address, aliases::I24},
        contracts::UniswapV3Pool::UniswapV3Pool::{Burn, Mint, Swap},
        serde_json::json,
        testlib::assert_json_matches,
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
            address: address!("0x0001fcbba8eb491c3ccfeddc5a5caba1a98c4c28"),
            tokens: vec![
                Token {
                    id: address!("0xbef81556ef066ec840a540595c8d12f516b6378f"),
                    decimals: 18,
                },
                Token {
                    id: address!("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                    decimals: 18,
                },
            ],
            state: PoolState {
                sqrt_price: U256::from(792216481398733702759960397_u128),
                liquidity: U256::from(303015134493562686441_u128),
                tick: -92110,
                liquidity_net: Arc::new(BTreeMap::from([
                    (-122070, 104713649338178916454i128),
                    (-77030, 1182024318125220460617i128),
                    (67260, 5812623076452005012674i128),
                ])),
                fee: Ratio::new(10_000u32, 1_000_000u32),
            },
            gas_stats: PoolStats {
                mean_gas: U256::from(300000),
            },
        };

        let serialized = serde_json::to_value(pool).unwrap();
        assert_json_matches!(json, serialized);
    }

    #[test]
    fn append_events_test_empty() {
        let pools = HashMap::from([(Address::with_last_byte(1), Default::default())]);
        let mut new_pools = pools.clone();
        let events = vec![];
        append_events(&mut new_pools, events);
        assert_eq!(new_pools, pools);
    }

    #[test]
    fn append_events_test_swap() {
        let address = Address::with_last_byte(1);
        let pool = Arc::new(PoolInfo {
            address,
            ..Default::default()
        });
        let mut pools = HashMap::from([(address, pool)]);

        let event = WithAddress::new(
            UniswapV3PoolEvents::Swap(Swap {
                sqrtPriceX96: U160::from(1),
                liquidity: 2u128,
                tick: I24::try_from(3).unwrap(),
                sender: Default::default(),
                recipient: Default::default(),
                amount0: Default::default(),
                amount1: Default::default(),
            }),
            address,
        );
        append_events(&mut pools, vec![event]);

        assert_eq!(pools[&address].state.tick, 3);
        assert_eq!(pools[&address].state.liquidity, U256::from(2));
        assert_eq!(pools[&address].state.sqrt_price, U256::from(1));
    }

    #[test]
    fn append_events_test_burn() {
        let address = Address::with_last_byte(1);
        let pool = Arc::new(PoolInfo {
            address,
            ..Default::default()
        });
        let mut pools = HashMap::from([(address, pool)]);

        // add first burn event
        let event = WithAddress::new(
            UniswapV3PoolEvents::Burn(Burn {
                tickLower: I24::try_from(100000).unwrap(),
                tickUpper: I24::try_from(110000).unwrap(),
                amount: 12345u128,
                owner: Default::default(),
                amount0: Default::default(),
                amount1: Default::default(),
            }),
            address,
        );
        append_events(&mut pools, vec![event]);
        assert_eq!(
            *pools[&address].state.liquidity_net,
            BTreeMap::from([(100_000, -12345i128), (110_000, 12345i128)])
        );

        // add second burn event
        let event = WithAddress::new(
            UniswapV3PoolEvents::Burn(Burn {
                tickLower: I24::try_from(105000).unwrap(),
                tickUpper: I24::try_from(110000).unwrap(),
                amount: 54321u128,
                owner: Default::default(),
                amount0: Default::default(),
                amount1: Default::default(),
            }),
            address,
        );
        append_events(&mut pools, vec![event]);
        assert_eq!(
            *pools[&address].state.liquidity_net,
            BTreeMap::from([
                (100_000, -12345i128),
                (105_000, -54321i128),
                (110_000, 66666i128)
            ])
        );
    }

    #[test]
    fn append_events_test_mint() {
        let address = Address::with_last_byte(1);
        let pool = Arc::new(PoolInfo {
            address,
            ..Default::default()
        });
        let mut pools = HashMap::from([(address, pool)]);

        // add first mint event
        let event = WithAddress::new(
            UniswapV3PoolEvents::Mint(Mint {
                tickLower: I24::try_from(100000).unwrap(),
                tickUpper: I24::try_from(110000).unwrap(),
                amount: 12345u128,
                owner: Default::default(),
                amount0: Default::default(),
                amount1: Default::default(),
                sender: Default::default(),
            }),
            address,
        );
        append_events(&mut pools, vec![event]);
        assert_eq!(
            *pools[&address].state.liquidity_net,
            BTreeMap::from([(100_000, 12345i128), (110_000, -12345i128)])
        );

        // add second burn event
        let event = WithAddress::new(
            UniswapV3PoolEvents::Mint(Mint {
                tickLower: I24::try_from(105000).unwrap(),
                tickUpper: I24::try_from(110000).unwrap(),
                amount: 54321u128,
                owner: Default::default(),
                amount0: Default::default(),
                amount1: Default::default(),
                sender: Default::default(),
            }),
            address,
        );
        append_events(&mut pools, vec![event]);
        assert_eq!(
            *pools[&address].state.liquidity_net,
            BTreeMap::from([
                (100_000, 12345i128),
                (105_000, 54321i128),
                (110_000, -66666i128)
            ])
        );
    }
}
