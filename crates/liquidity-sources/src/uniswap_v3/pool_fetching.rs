use {
    super::{
        BlockTarget,
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
    itertools::{Either, Itertools},
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

/// What `get` finds for a set of token pairs against the checkpoint.
#[derive(Default)]
struct CachedPools {
    /// Pools already present in the checkpoint cache.
    pools: HashMap<Address, Arc<PoolInfo>>,
    /// Registered pools not yet cached; fetched at the current block on the
    /// quote path and folded into the cache by the next maintenance run.
    missing: Vec<Address>,
    /// Block the cached pools are anchored at (0 if no pools are registered).
    block_number: u64,
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
        let mut registered_pools = source
            .get_registered_pools(BlockTarget::Number(target_block))
            .await?;
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
            .get_pools_with_ticks_by_ids(
                &pool_ids,
                BlockTarget::Number(registered_pools.fetched_block_number),
            )
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

    /// Returns cached pools for the pairs, plus the ids of any that exist but
    /// aren't cached yet. Misses are recorded for the next maintenance run.
    fn get(&self, token_pairs: &HashSet<TokenPair>) -> CachedPools {
        let mut pool_ids = token_pairs
            .iter()
            .filter_map(|pair| self.pools_by_token_pair.get(pair))
            .flatten()
            .peekable();

        match pool_ids.peek() {
            Some(_) => {
                let mut pools_checkpoint = self.pools_checkpoint.lock().unwrap();
                let (pools, missing): (HashMap<Address, Arc<PoolInfo>>, Vec<Address>) = pool_ids
                    .partition_map(|pool_id| match pools_checkpoint.pools.get(pool_id) {
                        Some(entry) => Either::Left((*pool_id, entry.clone())),
                        None => Either::Right(*pool_id),
                    });

                tracing::trace!("cache hit: {:?}, cache miss: {:?}", pools.keys(), missing);
                pools_checkpoint.missing_pools.extend(&missing);
                CachedPools {
                    pools,
                    missing,
                    block_number: pools_checkpoint.block_number,
                }
            }
            None => CachedPools::default(),
        }
    }

    /// Fetches and converts the given pools from the source at `target_block`,
    /// skipping any that can't be converted yet (e.g. missing ticks).
    async fn fetch_pools(
        &self,
        pool_ids: &[Address],
        target_block: BlockTarget,
    ) -> Result<Vec<(Address, Arc<PoolInfo>)>> {
        let pools_with_ticks = self
            .source
            .get_pools_with_ticks_by_ids(pool_ids, target_block)
            .await?;
        Ok(pools_with_ticks
            .pools
            .into_iter()
            .filter_map(|pool| {
                let id = pool.id;
                match PoolInfo::try_from(pool) {
                    Ok(info) => Some((id, Arc::new(info))),
                    Err(err) => {
                        tracing::debug!(?id, ?err, "skipping pool missing tick data");
                        None
                    }
                }
            })
            .collect())
    }

    /// Fetches the given missing pools at the source's head, but only when the
    /// source can serve that cheaply ([`V3PoolDataSource::serves_on_demand`],
    /// i.e. the indexer). For the subgraph this is a no-op: the misses are
    /// already recorded for the maintenance run, and a synchronous at-head
    /// fetch there does a safe-block lookup + paginated pool/tick queries
    /// per pair and would stall quotes on a cold cache. A fetch failure
    /// yields fewer pools rather than failing the whole quote.
    async fn fetch_missing_on_demand(&self, missing: &[Address]) -> Vec<(Address, Arc<PoolInfo>)> {
        if missing.is_empty() || !self.source.serves_on_demand() {
            return Vec::new();
        }
        self.fetch_pools(missing, BlockTarget::Latest)
            .await
            .inspect_err(|err| tracing::debug!(?err, "on-demand pool fetch failed"))
            .unwrap_or_default()
    }

    /// Fetches the pools flagged missing by `get` at the checkpoint block and
    /// caches them. Runs from periodic maintenance.
    async fn update_missing_pools(&self) -> Result<()> {
        // Clone out and drop the lock before the async fetch.
        let (missing, block_number) = {
            let checkpoint = self.pools_checkpoint.lock().unwrap();
            (checkpoint.missing_pools.clone(), checkpoint.block_number)
        };
        if missing.is_empty() {
            return Ok(());
        }

        let fetched = self
            .fetch_pools(&Vec::from_iter(missing), BlockTarget::Number(block_number))
            .await?;
        let mut checkpoint = self.pools_checkpoint.lock().unwrap();
        for (id, info) in fetched {
            checkpoint.missing_pools.remove(&id);
            checkpoint.pools.insert(id, info);
        }
        if !checkpoint.missing_pools.is_empty() {
            tracing::warn!(
                remaining = checkpoint.missing_pools.len(),
                "not all missing pools updated"
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
        let CachedPools {
            pools: mut checkpoint,
            missing,
            block_number: checkpoint_block_number,
        } = self.checkpoint.get(token_pairs);

        // No pools registered for these pairs: nothing to fetch or replay.
        if checkpoint.is_empty() && missing.is_empty() {
            return Ok(Vec::new());
        }

        if block_number > checkpoint_block_number {
            let block_range = RangeInclusive::try_new(checkpoint_block_number + 1, block_number)?;
            let events = self.events.lock().await.store().get_events(block_range);
            append_events(&mut checkpoint, events);
        }

        // The warm cache only holds the top pools by raw liquidity, so many
        // registered pairs are absent. When the source can serve them cheaply
        // (the indexer), fetch the misses at its head — not the checkpoint block,
        // which tracks latest-minus-reorg and can sit ahead of the indexer's
        // served head, so waiting on it would hang the quote. They come back
        // current, so merge after the replay instead of replaying them. For the
        // subgraph this is a no-op and the misses wait for the maintenance run.
        checkpoint.extend(self.checkpoint.fetch_missing_on_demand(&missing).await);

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

    /// Serves a fixed set of pools (with ticks) from
    /// `get_pools_with_ticks_by_ids` so the on-demand fetch path can be
    /// exercised without a real source. `served_block` models the indexer's
    /// head: a request for a higher `target_block` fails, mirroring the real
    /// client's `wait_until` blocking on a block the indexer hasn't reached.
    struct StubSource {
        with_ticks: HashMap<Address, PoolData>,
        served_block: u64,
        serves_on_demand: bool,
    }

    impl StubSource {
        fn new(pools: impl IntoIterator<Item = PoolData>) -> Self {
            Self {
                with_ticks: pools.into_iter().map(|p| (p.id, p)).collect(),
                served_block: u64::MAX,
                serves_on_demand: true,
            }
        }
    }

    #[async_trait::async_trait]
    impl V3PoolDataSource for StubSource {
        fn serves_on_demand(&self) -> bool {
            self.serves_on_demand
        }

        async fn get_registered_pools(
            &self,
            _target_block: BlockTarget,
        ) -> Result<crate::uniswap_v3::graph_api::RegisteredPools> {
            Ok(Default::default())
        }

        async fn get_pools_with_ticks_by_ids(
            &self,
            ids: &[Address],
            target_block: BlockTarget,
        ) -> Result<crate::uniswap_v3::graph_api::PoolsWithTicks> {
            let target_block = match target_block {
                BlockTarget::Latest => self.served_block,
                BlockTarget::Number(n) => n,
            };
            anyhow::ensure!(
                target_block <= self.served_block,
                "indexer at {} hasn't reached target block {target_block}",
                self.served_block,
            );
            let pools = ids
                .iter()
                .filter_map(|id| self.with_ticks.get(id).cloned())
                .collect();
            Ok(crate::uniswap_v3::graph_api::PoolsWithTicks {
                fetched_block_number: self.served_block,
                pools,
            })
        }
    }

    fn pool_with_ticks(id: Address, token0: Address, token1: Address) -> PoolData {
        PoolData {
            id,
            token0: Token {
                id: token0,
                decimals: 6,
            },
            token1: Token {
                id: token1,
                decimals: 18,
            },
            fee_tier: U256::from(3000),
            liquidity: U256::from(1_000_000u64),
            sqrt_price: U256::from(1u64),
            tick: 0,
            ticks: Some(vec![crate::uniswap_v3::graph_api::TickData {
                tick_idx: -100,
                liquidity_net: 1_000,
                pool_address: id,
            }]),
            block_number: 100,
        }
    }

    fn handler(source: StubSource, checkpoint: PoolsCheckpoint) -> PoolsCheckpointHandler {
        PoolsCheckpointHandler {
            source: Arc::new(source),
            pools_by_token_pair: HashMap::new(),
            pools_checkpoint: Mutex::new(checkpoint),
        }
    }

    /// A pool registered for a pair but absent from the warm cache is returned
    /// in `missing` (not `pools`), so the fetch path knows to resolve it.
    #[test]
    fn get_flags_registered_uncached_pool_as_missing() {
        let token0 = Address::with_last_byte(1);
        let token1 = Address::with_last_byte(2);
        let pair = TokenPair::new(token0, token1).unwrap();
        let pool = Address::with_last_byte(9);

        let mut handler = handler(
            StubSource::new([]),
            PoolsCheckpoint {
                pools: HashMap::new(),
                block_number: 100,
                missing_pools: HashSet::new(),
            },
        );
        handler.pools_by_token_pair = HashMap::from([(pair, vec![pool])]);

        let CachedPools { pools, missing, .. } = handler.get(&HashSet::from([pair]));
        assert!(pools.is_empty());
        assert_eq!(missing, vec![pool]);
    }

    /// The on-demand path must not block on the checkpoint block (which can sit
    /// persistently ahead of the indexer's served block); it fetches at the
    /// indexer's latest block. A source that errors for any block above its
    /// head still yields the pool via a `BlockTarget::Latest` fetch.
    #[tokio::test]
    async fn on_demand_does_not_wait_for_future_block() {
        let token0 = Address::with_last_byte(1);
        let token1 = Address::with_last_byte(2);
        let pool = Address::with_last_byte(9);

        let mut source = StubSource::new([pool_with_ticks(pool, token0, token1)]);
        source.served_block = 50; // indexer behind the checkpoint below

        let handler = handler(
            source,
            PoolsCheckpoint {
                pools: HashMap::new(),
                block_number: 100, // checkpoint ahead of the indexer
                missing_pools: HashSet::new(),
            },
        );

        // Latest serves at-head, so it succeeds despite served_block < checkpoint.
        let fetched = handler
            .fetch_pools(&[pool], BlockTarget::Latest)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].0, pool);

        // Fetching at the checkpoint block would fail (indexer hasn't reached it).
        assert!(
            handler
                .fetch_pools(&[pool], BlockTarget::Number(100))
                .await
                .is_err()
        );
    }

    /// The on-demand fetch runs only for sources that can serve it cheaply: an
    /// indexer-capable source resolves the miss, a subgraph-capable one returns
    /// nothing (the miss is left for the maintenance run).
    #[tokio::test]
    async fn fetch_missing_on_demand_gated_by_capability() {
        let token0 = Address::with_last_byte(1);
        let token1 = Address::with_last_byte(2);
        let pool = Address::with_last_byte(9);
        let checkpoint = || PoolsCheckpoint {
            pools: HashMap::new(),
            block_number: 100,
            missing_pools: HashSet::new(),
        };

        let indexer = handler(
            StubSource::new([pool_with_ticks(pool, token0, token1)]),
            checkpoint(),
        );
        assert_eq!(indexer.fetch_missing_on_demand(&[pool]).await.len(), 1);

        let mut subgraph_src = StubSource::new([pool_with_ticks(pool, token0, token1)]);
        subgraph_src.serves_on_demand = false;
        let subgraph = handler(subgraph_src, checkpoint());
        assert!(subgraph.fetch_missing_on_demand(&[pool]).await.is_empty());
    }

    /// Unknown pairs have no registered pools, so `get` reports block 0 with
    /// nothing cached or missing — the signal `fetch` uses to skip the replay.
    #[test]
    fn get_reports_zero_block_for_unknown_pairs() {
        let handler = handler(
            StubSource::new([]),
            PoolsCheckpoint {
                pools: HashMap::new(),
                block_number: 100,
                missing_pools: HashSet::new(),
            },
        );
        let pair = TokenPair::new(Address::with_last_byte(1), Address::with_last_byte(2)).unwrap();
        let CachedPools {
            pools,
            missing,
            block_number,
        } = handler.get(&HashSet::from([pair]));
        assert!(pools.is_empty());
        assert!(missing.is_empty());
        assert_eq!(block_number, 0);
    }

    /// A pool that can't be converted (e.g. ticks not yet available) is skipped
    /// rather than failing the whole batch; the convertible pool is returned.
    #[tokio::test]
    async fn fetch_pools_skips_unconvertible_pool() {
        let token0 = Address::with_last_byte(1);
        let token1 = Address::with_last_byte(2);
        let good = Address::with_last_byte(9);
        let bad = Address::with_last_byte(10);

        let mut bad_pool = pool_with_ticks(bad, token0, token1);
        bad_pool.ticks = None; // PoolInfo::try_from fails on missing ticks

        let handler = handler(
            StubSource::new([pool_with_ticks(good, token0, token1), bad_pool]),
            PoolsCheckpoint {
                pools: HashMap::new(),
                block_number: 100,
                missing_pools: HashSet::new(),
            },
        );

        let fetched = handler
            .fetch_pools(&[good, bad], BlockTarget::Latest)
            .await
            .unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].0, good);
    }
}
