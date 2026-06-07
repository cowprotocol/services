//! HTTP client for the Uniswap V3 pool-indexer service. Implements
//! [`V3PoolDataSource`] so the driver can swap this in place of the subgraph
//! client without touching anything else.
//!
//! The pool-indexer doesn't support historical queries; it always serves
//! at-head data. To give callers a consistent snapshot, each method takes a
//! `target_block` and blocks (via [`PoolIndexerClient::wait_until`]) until the
//! indexer's envelope reports a block at or after it. The actual snapshot
//! block — which can be later than `target_block` if the indexer advanced
//! during the call — is returned alongside the data so callers can anchor
//! their event-replay at the right place.

use {
    crate::uniswap_v3::{
        V3PoolDataSource,
        graph_api::{PoolData, PoolsWithTicks, RegisteredPools, TickData, Token},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    async_trait::async_trait,
    chain::Chain,
    itertools::Itertools,
    num::BigInt,
    number::serialization::HexOrDecimalU256,
    reqwest::{Client, Url},
    serde::{Deserialize, Deserializer, de},
    serde_with::{DisplayFromStr, serde_as},
    std::{collections::HashMap, time::Duration},
};

/// How often [`PoolIndexerClient::wait_until`] polls `/pools?limit=1` while
/// waiting for the indexer's head to catch up.
const WAIT_UNTIL_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Pool-indexer's server-side cap on `pool_ids=` query param size; keep our
/// per-request chunk at or below this.
const POOL_IDS_PER_REQUEST: usize = 500;

/// Pool-indexer's server-side cap on `limit=` for listing pools.
const LIST_PAGE_SIZE: u64 = 5000;

pub struct PoolIndexerClient {
    /// Service root (e.g. `http://pool-indexer/`).
    base_url: Url,
    http: Client,
    /// Upper bound on a single [`Self::wait_until`] call. Sized per network
    /// to comfortably exceed the worst-case first-deploy seed time.
    wait_until_timeout: Duration,
}

impl PoolIndexerClient {
    pub fn new(base_url: Url, chain: Chain, http: Client, wait_until_timeout: Duration) -> Self {
        let prefix = format!("api/v1/{}/uniswap/v3/", chain.as_str());
        let base_url = url_join(&base_url, &prefix);
        Self {
            base_url,
            http,
            wait_until_timeout,
        }
    }

    fn path(&self, suffix: &str) -> Url {
        url_join(&self.base_url, suffix)
    }
}

/// Joins path onto URL with exactly one slash. Inlined to avoid a
/// `shared` → `liquidity-sources` dep cycle.
fn url_join(url: &Url, mut path: &str) -> Url {
    let mut url = url.to_string();
    while url.ends_with('/') {
        url.pop();
    }
    while path.starts_with('/') {
        path = &path[1..];
    }
    Url::parse(&format!("{url}/{path}")).expect("constructed URL is valid")
}

#[derive(Deserialize)]
struct PoolsResponse {
    block_number: u64,
    pools: Vec<IndexerPool>,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[serde_as]
#[derive(Deserialize)]
struct IndexerPool {
    id: Address,
    token0: IndexerToken,
    token1: IndexerToken,
    #[serde_as(as = "HexOrDecimalU256")]
    fee_tier: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    liquidity: U256,
    #[serde_as(as = "HexOrDecimalU256")]
    sqrt_price: U256,
    tick: i32,
}

#[derive(Deserialize)]
struct IndexerToken {
    id: Address,
    #[serde(default)]
    decimals: Option<u8>,
}

#[derive(Deserialize)]
struct BulkTicksResponse {
    /// Deserialised from the wire `pools: [{ pool, ticks }, …]` array into a
    /// `HashMap` keyed by pool address. A duplicate `pool` key fails the
    /// deserialisation rather than silently overwriting — the server-side
    /// API contract is one entry per requested pool.
    #[serde(deserialize_with = "deserialize_ticks_by_pool")]
    pools: HashMap<Address, Vec<IndexerTick>>,
}

#[derive(Deserialize)]
struct IndexerPoolTicks {
    pool: Address,
    ticks: Vec<IndexerTick>,
}

fn deserialize_ticks_by_pool<'de, D: Deserializer<'de>>(
    de: D,
) -> Result<HashMap<Address, Vec<IndexerTick>>, D::Error> {
    let entries = Vec::<IndexerPoolTicks>::deserialize(de)?;
    let mut out = HashMap::with_capacity(entries.len());
    for IndexerPoolTicks { pool, ticks } in entries {
        if out.insert(pool, ticks).is_some() {
            return Err(de::Error::custom(format!(
                "pool-indexer returned duplicate ticks for pool {pool:#x}",
            )));
        }
    }
    Ok(out)
}

#[serde_as]
#[derive(Deserialize)]
struct IndexerTick {
    tick_idx: i32,
    #[serde_as(as = "DisplayFromStr")]
    liquidity_net: BigInt,
}

/// Drops pools where either token's `decimals` is missing. Treating missing
/// as `0` would mis-scale prices by 10^18; fail closed until the indexer's
/// decimals backfill catches up.
///
/// On a fresh deploy the indexer's backfill can take a few minutes, during
/// which every page can carry hundreds of decimals-missing pools — so we
/// aggregate the drops into a single `debug!` per call rather than logging
/// `warn!` per pool. Steady-state this should be a no-op.
fn drop_pools_missing_decimals(mut pools: Vec<IndexerPool>) -> Vec<IndexerPool> {
    let total = pools.len();
    pools.retain(|p| p.token0.decimals.is_some() && p.token1.decimals.is_some());
    let dropped = total - pools.len();
    if dropped > 0 {
        tracing::debug!(
            dropped,
            total,
            "pool-indexer returned pools missing token decimals; filtered out",
        );
    }
    pools
}

impl TryFrom<IndexerPool> for PoolData {
    type Error = anyhow::Error;

    fn try_from(pool: IndexerPool) -> Result<Self> {
        let token0_decimals = pool
            .token0
            .decimals
            .context("BUG: missing token0 decimals after pools_tokens_have_decimals filter")?;
        let token1_decimals = pool
            .token1
            .decimals
            .context("BUG: missing token1 decimals after pools_tokens_have_decimals filter")?;
        Ok(Self {
            id: pool.id,
            token0: Token {
                id: pool.token0.id,
                decimals: token0_decimals,
            },
            token1: Token {
                id: pool.token1.id,
                decimals: token1_decimals,
            },
            fee_tier: pool.fee_tier,
            liquidity: pool.liquidity,
            sqrt_price: pool.sqrt_price,
            tick: BigInt::from(pool.tick),
            ticks: None,
        })
    }
}

impl IndexerTick {
    fn into_tick_data(self, pool_address: Address) -> TickData {
        TickData {
            tick_idx: BigInt::from(self.tick_idx),
            liquidity_net: self.liquidity_net,
            pool_address,
        }
    }
}

impl PoolIndexerClient {
    /// Blocks until the indexer's `/pools` envelope reports `block_number >=
    /// target_block`, capped at [`Self::wait_until_timeout`]. Polls every
    /// [`WAIT_UNTIL_POLL_INTERVAL`]; returns immediately if the indexer is
    /// already in range. The probe is a `?limit=1` listing so the round-trip
    /// stays cheap.
    ///
    /// `503 Service Unavailable` is treated as "indexer still bootstrapping"
    /// (it returns 503 until the first checkpoint exists) and the loop
    /// keeps polling. Every other non-2xx is propagated as an error — those
    /// are genuine problems the caller should see.
    async fn wait_until(&self, target_block: u64) -> Result<()> {
        let deadline = std::time::Instant::now() + self.wait_until_timeout;
        let mut last_observed: Option<u64> = None;
        let mut interval = tokio::time::interval(WAIT_UNTIL_POLL_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            match self.fetch_pools_page(1, None).await? {
                None => tracing::debug!(
                    %target_block,
                    "pool-indexer not ready yet (503); waiting",
                ),
                Some(probe) => {
                    if probe.block_number >= target_block {
                        return Ok(());
                    }
                    last_observed = Some(probe.block_number);
                    tracing::debug!(
                        indexer_block = probe.block_number,
                        %target_block,
                        "pool-indexer not yet at target block; waiting",
                    );
                }
            }
            if std::time::Instant::now() >= deadline {
                anyhow::bail!(
                    "pool-indexer wait_until exceeded {:?} waiting for block {target_block}; last \
                     observed indexer block: {last_observed:?}",
                    self.wait_until_timeout,
                );
            }
        }
    }

    /// Issues `GET /pools?limit=N[&after=cursor]` and parses the envelope.
    /// `None` means the indexer responded 503 (still bootstrapping); the
    /// caller decides whether to retry or fail.
    async fn fetch_pools_page(
        &self,
        limit: u64,
        cursor: Option<&str>,
    ) -> Result<Option<PoolsResponse>> {
        let mut url = self.path("pools");
        url.query_pairs_mut()
            .append_pair("limit", &limit.to_string());
        if let Some(c) = cursor {
            url.query_pairs_mut().append_pair("after", c);
        }
        let resp = self.http.get(url).send().await.context("GET /pools")?;
        if resp.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            return Ok(None);
        }
        let page: PoolsResponse = resp
            .error_for_status()
            .context("/pools HTTP status")?
            .json()
            .await
            .context("/pools body")?;
        Ok(Some(page))
    }
}

#[async_trait]
impl V3PoolDataSource for PoolIndexerClient {
    async fn get_registered_pools(&self, target_block: u64) -> Result<RegisteredPools> {
        self.wait_until(target_block).await?;
        // We pin the snapshot to the first page's block_number; later pages
        // may report a higher block — bounded drift, picked up by the
        // driver's event replay.
        let mut cursor: Option<String> = None;
        let mut pools: Vec<PoolData> = Vec::new();
        let mut fetched_block_number: Option<u64> = None;
        loop {
            let page = self
                .fetch_pools_page(LIST_PAGE_SIZE, cursor.as_deref())
                .await?
                .context("pool-indexer returned 503 after wait_until")?;

            fetched_block_number.get_or_insert(page.block_number);
            // Skip zero-liquidity pools (fully-burned LP, never-minted, etc.)
            let mut liq_filtered = page.pools;
            liq_filtered.retain(|p| !p.liquidity.is_zero());
            let filtered = drop_pools_missing_decimals(liq_filtered)
                .into_iter()
                .map(PoolData::try_from)
                .collect::<Result<Vec<_>>>()?;
            pools.extend(filtered);
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(RegisteredPools {
            fetched_block_number: fetched_block_number.context("pool-indexer returned no pages")?,
            pools,
        })
    }

    async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[Address],
        target_block: u64,
    ) -> Result<PoolsWithTicks> {
        self.wait_until(target_block).await?;

        if ids.is_empty() {
            return Ok(PoolsWithTicks {
                fetched_block_number: target_block,
                pools: Vec::new(),
            });
        }

        let mut out: Vec<PoolData> = Vec::with_capacity(ids.len());
        let mut fetched_block_number: Option<u64> = None;
        for batch in ids.chunks(POOL_IDS_PER_REQUEST) {
            let (pools_page, ticks_by_pool) = futures::try_join!(
                fetch_pools_by_ids(self, batch),
                fetch_ticks_by_pool_ids(self, batch),
            )?;

            fetched_block_number.get_or_insert(pools_page.block_number);
            for mut pool in pools_page.pools {
                if let Some(ticks) = ticks_by_pool.get(&pool.id) {
                    pool.ticks = Some(ticks.clone());
                }
                out.push(pool);
            }
        }
        Ok(PoolsWithTicks {
            // For non-empty `ids` the loop always runs at least once, so
            // `fetched_block_number` is always populated. Fall back to
            // `target_block` defensively rather than panicking.
            fetched_block_number: fetched_block_number.unwrap_or(target_block),
            pools: out,
        })
    }
}

/// First-page result of a `pools/by-ids` fetch, plus the indexer's response
/// envelope block_number captured for the caller's event-replay anchor.
struct PoolsByIdsPage {
    block_number: u64,
    pools: Vec<PoolData>,
}

fn ids_param(ids: &[Address]) -> String {
    ids.iter().map(const_hex::encode_prefixed).join(",")
}

async fn fetch_pools_by_ids(client: &PoolIndexerClient, ids: &[Address]) -> Result<PoolsByIdsPage> {
    let mut url = client.path("pools/by-ids");
    url.query_pairs_mut()
        .append_pair("pool_ids", &ids_param(ids));
    let resp: PoolsResponse = client
        .http
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET /pools/by-ids?pool_ids <{} ids>", ids.len()))?
        .error_for_status()
        .context("pools-by-ids HTTP status")?
        .json()
        .await
        .context("pools-by-ids body")?;
    let pools = drop_pools_missing_decimals(resp.pools)
        .into_iter()
        .map(PoolData::try_from)
        .collect::<Result<Vec<_>>>()?;
    Ok(PoolsByIdsPage {
        block_number: resp.block_number,
        pools,
    })
}

async fn fetch_ticks_by_pool_ids(
    client: &PoolIndexerClient,
    ids: &[Address],
) -> Result<HashMap<Address, Vec<TickData>>> {
    let mut url = client.path("pools/ticks");
    url.query_pairs_mut()
        .append_pair("pool_ids", &ids_param(ids));
    let resp: BulkTicksResponse = client
        .http
        .get(url)
        .send()
        .await
        .context("GET /pools/ticks")?
        .error_for_status()
        .context("bulk-ticks HTTP status")?
        .json()
        .await
        .context("bulk-ticks body")?;
    Ok(resp
        .pools
        .into_iter()
        .map(|(pool, ticks)| {
            let mapped: Vec<_> = ticks.into_iter().map(|t| t.into_tick_data(pool)).collect();
            (pool, mapped)
        })
        .collect())
}
