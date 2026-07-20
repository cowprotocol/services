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
        BlockTarget,
        V3PoolDataSource,
        graph_api::{PoolData, PoolsWithTicks, RegisteredPools, TickData, Token},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    async_trait::async_trait,
    chain::Chain,
    itertools::Itertools,
    number::serialization::HexOrDecimalU256,
    reqwest::{Client, Url},
    serde::{Deserialize, Deserializer, de},
    serde_with::{DisplayFromStr, serde_as},
    std::{collections::HashMap, time::Duration},
};

/// Poll interval for [`PoolIndexerClient::wait_until`].
const WAIT_UNTIL_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Cap on a single `wait_until` call. Bootstrap now runs as a separate
/// initContainer, so the serve container is up within seconds — this only
/// covers the indexer coming up at startup, not the old cold-bootstrap time.
const WAIT_UNTIL_TIMEOUT: Duration = Duration::from_secs(60);

/// Matches the server-side `MAX_POOL_IDS_PER_REQUEST`.
const POOL_IDS_PER_REQUEST: usize = 500;

/// Matches the server-side `MAX_PAGE_LIMIT`.
const LIST_PAGE_SIZE: u64 = 5000;

pub struct PoolIndexerClient {
    base_url: Url,
    http: Client,
}

impl PoolIndexerClient {
    pub fn new(base_url: Url, chain: Chain, http: Client) -> Self {
        let prefix = format!("api/v1/{}/uniswap/v3/", chain.as_str());
        let base_url = url_join(&base_url, &prefix);
        Self { base_url, http }
    }

    fn path(&self, suffix: &str) -> Url {
        url_join(&self.base_url, suffix)
    }
}

/// Joins `path` onto `url` with exactly one slash between them. Reusing
/// `Url::join` is unreliable here because a base URL without a trailing
/// slash drops its last path segment (RFC 3986 path resolution), and the
/// pool-indexer base URL comes from operator config which may or may not
/// have one.
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
    /// Block of the envelope. Compared against the paired `/pools/by-ids`
    /// block in `fetch_batch_consistent` so the caller never mixes pool
    /// state and tick state from different blocks.
    block_number: u64,
    /// Wire shape is `pools: [{ pool, ticks }, …]`; we re-index by pool
    /// address. Duplicate pool keys fail deserialisation rather than
    /// silently overwriting — the API contract is one entry per pool.
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
#[derive(Clone, Deserialize)]
struct IndexerTick {
    tick_idx: i32,
    #[serde_as(as = "DisplayFromStr")]
    liquidity_net: i128,
}

/// Drops pools missing either token's `decimals`. Treating missing as `0`
/// would mis-scale prices, so we fail closed and wait for the indexer's
/// decimals backfill.
///
/// On a fresh deploy the backfill can take minutes and each page may
/// carry hundreds of unfilled pools, so we log a single `debug!` summary
/// per call. Steady-state this should be a no-op.
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

impl IndexerPool {
    /// Stamps the envelope's `block_number` onto the pool so the driver
    /// can anchor per-pool event replay. Bails if `decimals` is missing —
    /// run [`drop_pools_missing_decimals`] first.
    fn into_pool_data(self, block_number: u64) -> Result<PoolData> {
        let token0_decimals = self
            .token0
            .decimals
            .context("BUG: missing token0 decimals after drop_pools_missing_decimals filter")?;
        let token1_decimals = self
            .token1
            .decimals
            .context("BUG: missing token1 decimals after drop_pools_missing_decimals filter")?;
        Ok(PoolData {
            id: self.id,
            token0: Token {
                id: self.token0.id,
                decimals: token0_decimals,
            },
            token1: Token {
                id: self.token1.id,
                decimals: token1_decimals,
            },
            fee_tier: self.fee_tier,
            liquidity: self.liquidity,
            sqrt_price: self.sqrt_price,
            tick: self.tick,
            ticks: None,
            block_number,
        })
    }
}

impl IndexerTick {
    fn into_tick_data(self, pool_address: Address) -> TickData {
        TickData {
            tick_idx: self.tick_idx,
            liquidity_net: self.liquidity_net,
            pool_address,
        }
    }
}

impl PoolIndexerClient {
    /// Polls `/pools?limit=1` every [`WAIT_UNTIL_POLL_INTERVAL`] until the
    /// envelope reports `block_number >= target_block`. Returns
    /// immediately if already there; bails after [`WAIT_UNTIL_TIMEOUT`].
    /// [`BlockTarget::Latest`] serves at-head without waiting.
    ///
    /// `503` is treated as "still bootstrapping" and the loop keeps
    /// polling. Other non-2xx statuses propagate as errors.
    async fn wait_until(&self, target_block: BlockTarget) -> Result<()> {
        let target_block = match target_block {
            BlockTarget::Latest => return Ok(()),
            BlockTarget::Number(n) => n,
        };
        let deadline = std::time::Instant::now() + WAIT_UNTIL_TIMEOUT;
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
                    WAIT_UNTIL_TIMEOUT,
                );
            }
        }
    }

    /// `GET /pools?limit=N[&after=cursor]`. `None` means 503 (still
    /// bootstrapping) — the caller decides whether to retry or fail.
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
    /// The indexer serves at-head from its own DB, so an on-demand fetch is
    /// cheap enough for the quote path.
    fn serves_on_demand(&self) -> bool {
        true
    }

    async fn get_registered_pools(&self, target_block: BlockTarget) -> Result<RegisteredPools> {
        self.wait_until(target_block).await?;
        // The indexer can advance between pages, so each pool carries the
        // block of the page it arrived in. The envelope's
        // `fetched_block_number` is the first page's block — a
        // conservative lower bound for callers that only need a global
        // "fresh as of" anchor.
        let mut cursor: Option<String> = None;
        let mut pools: Vec<PoolData> = Vec::new();
        let mut fetched_block_number: Option<u64> = None;
        loop {
            let page = self
                .fetch_pools_page(LIST_PAGE_SIZE, cursor.as_deref())
                .await?
                .context("pool-indexer returned 503 after wait_until")?;

            let page_block = page.block_number;
            fetched_block_number.get_or_insert(page_block);
            // Drop zero-liquidity pools (matches the subgraph backend's
            // filter, see `graph_api::get_pools`).
            let mut liq_filtered = page.pools;
            liq_filtered.retain(|p| !p.liquidity.is_zero());
            let filtered = drop_pools_missing_decimals(liq_filtered)
                .into_iter()
                .map(|p| p.into_pool_data(page_block))
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
        target_block: BlockTarget,
    ) -> Result<PoolsWithTicks> {
        self.wait_until(target_block).await?;

        if ids.is_empty() {
            return Ok(PoolsWithTicks::default());
        }

        let mut out: Vec<PoolData> = Vec::with_capacity(ids.len());
        let mut fetched_block_number: Option<u64> = None;
        for batch in ids.chunks(POOL_IDS_PER_REQUEST) {
            let (pools_page, ticks_response) = self.fetch_batch_consistent(batch).await?;

            let batch_block = pools_page.block_number;
            fetched_block_number.get_or_insert(batch_block);
            for indexer_pool in pools_page.pools {
                let pool_id = indexer_pool.id;
                let mut pool = indexer_pool.into_pool_data(batch_block)?;
                if let Some(ticks) = ticks_response.pools.get(&pool_id) {
                    pool.ticks = Some(
                        ticks
                            .iter()
                            .map(|t| t.clone().into_tick_data(pool_id))
                            .collect(),
                    );
                }
                out.push(pool);
            }
        }
        Ok(PoolsWithTicks {
            fetched_block_number: fetched_block_number.context(
                "BUG: pool-indexer returned no batches for non-empty `ids` — empty short-circuits \
                 above",
            )?,
            pools: out,
        })
    }
}

/// Capped at 3 — mismatches are rare and typically resolve on the next attempt.
const BATCH_CONSISTENCY_MAX_RETRIES: usize = 3;

/// Wire-form `pools/by-ids` response. The caller stamps `block_number`
/// onto each `PoolData` during conversion.
struct PoolsByIdsPage {
    block_number: u64,
    pools: Vec<IndexerPool>,
}

fn ids_param(ids: &[Address]) -> String {
    ids.iter().map(const_hex::encode_prefixed).join(",")
}

impl PoolIndexerClient {
    /// Fetches pools + ticks for `ids` at a single indexer block. The two
    /// HTTP endpoints can drift if a chunk commits between them; we retry
    /// on mismatch up to [`BATCH_CONSISTENCY_MAX_RETRIES`]. On success
    /// both responses share a `block_number`.
    async fn fetch_batch_consistent(
        &self,
        ids: &[Address],
    ) -> Result<(PoolsByIdsPage, BulkTicksResponse)> {
        for attempt in 0..BATCH_CONSISTENCY_MAX_RETRIES {
            let (pools_page, ticks_response) = futures::try_join!(
                fetch_pools_by_ids(self, ids),
                fetch_ticks_by_pool_ids(self, ids),
            )?;
            if pools_page.block_number == ticks_response.block_number {
                return Ok((pools_page, ticks_response));
            }
            tracing::warn!(
                attempt,
                pools_block = pools_page.block_number,
                ticks_block = ticks_response.block_number,
                "pool-indexer block mismatch between pools and ticks responses; retrying",
            );
        }
        anyhow::bail!(
            "pool-indexer returned mismatched pools-vs-ticks blocks after \
             {BATCH_CONSISTENCY_MAX_RETRIES} retries"
        )
    }
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
    Ok(PoolsByIdsPage {
        block_number: resp.block_number,
        pools: drop_pools_missing_decimals(resp.pools),
    })
}

async fn fetch_ticks_by_pool_ids(
    client: &PoolIndexerClient,
    ids: &[Address],
) -> Result<BulkTicksResponse> {
    let mut url = client.path("pools/ticks");
    url.query_pairs_mut()
        .append_pair("pool_ids", &ids_param(ids));
    client
        .http
        .get(url)
        .send()
        .await
        .context("GET /pools/ticks")?
        .error_for_status()
        .context("bulk-ticks HTTP status")?
        .json()
        .await
        .context("bulk-ticks body")
}
