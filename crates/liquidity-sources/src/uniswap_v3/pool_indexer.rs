//! HTTP client for CoW Protocol's own pool-indexer service. Implements
//! [`V3PoolDataSource`] so the driver can swap this in place of the subgraph
//! client without touching anything else.
//!
//! The pool-indexer always returns at-head data — it doesn't support
//! historical queries. `block_number` arguments are ignored; the block
//! actually served is returned in the response envelope. For the driver's
//! current use this is fine (see design discussion around cold_seeder and
//! the baseline solver's eth_call delegation).

use {
    crate::uniswap_v3::{
        V3PoolDataSource,
        graph_api::{PoolData, RegisteredPools, TickData, Token},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    async_trait::async_trait,
    chain::Chain,
    num::BigInt,
    reqwest::{Client, Url},
    serde::Deserialize,
    std::{collections::HashMap, str::FromStr},
};

/// Pool-indexer's server-side cap on `pool_ids=` query param size; keep our
/// per-request chunk at or below this.
const POOL_IDS_PER_REQUEST: usize = 500;

/// Pool-indexer's server-side cap on `limit=` for listing pools.
const LIST_PAGE_SIZE: u64 = 5000;

pub struct PoolIndexerClient {
    /// Service root (e.g. `http://pool-indexer/`).
    base_url: Url,
    http: Client,
}

impl PoolIndexerClient {
    pub fn new(base_url: Url, chain: Chain, http: Client) -> Result<Self> {
        // `Url::join` replaces the last path segment unless the base ends in
        // a `/`. Build `<service-root>/api/v1/<network>/uniswap/v3/` once so
        // every `path()` call behaves like "append".
        let prefix = format!("api/v1/{}/uniswap/v3/", chain.slug());
        let with_trailing_slash = if base_url.path().ends_with('/') {
            base_url
        } else {
            let mut u = base_url;
            let p = format!("{}/", u.path());
            u.set_path(&p);
            u
        };
        let base_url = with_trailing_slash
            .join(&prefix)
            .with_context(|| format!("joining {prefix} onto {with_trailing_slash}"))?;
        Ok(Self { base_url, http })
    }

    fn path(&self, suffix: &str) -> Result<Url> {
        self.base_url
            .join(suffix)
            .with_context(|| format!("joining {suffix} onto {}", self.base_url))
    }
}

#[derive(Deserialize)]
struct PoolsResponse {
    block_number: u64,
    pools: Vec<IndexerPool>,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct IndexerPool {
    id: Address,
    token0: IndexerToken,
    token1: IndexerToken,
    fee_tier: String,
    liquidity: String,
    sqrt_price: String,
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
    pools: Vec<IndexerPoolTicks>,
}

#[derive(Deserialize)]
struct IndexerPoolTicks {
    pool: Address,
    ticks: Vec<IndexerTick>,
}

#[derive(Deserialize)]
struct IndexerTick {
    tick_idx: i32,
    liquidity_net: String,
}

impl TryFrom<IndexerPool> for PoolData {
    type Error = anyhow::Error;

    fn try_from(pool: IndexerPool) -> Result<Self> {
        Ok(Self {
            id: pool.id,
            token0: Token {
                id: pool.token0.id,
                decimals: pool.token0.decimals.unwrap_or(0),
            },
            token1: Token {
                id: pool.token1.id,
                decimals: pool.token1.decimals.unwrap_or(0),
            },
            fee_tier: U256::from_str(&pool.fee_tier).context("parse fee_tier")?,
            liquidity: U256::from_str(&pool.liquidity).context("parse liquidity")?,
            sqrt_price: U256::from_str(&pool.sqrt_price).context("parse sqrt_price")?,
            tick: BigInt::from(pool.tick),
            ticks: None,
        })
    }
}

impl IndexerTick {
    fn into_tick_data(self, pool_address: Address) -> Result<TickData> {
        Ok(TickData {
            id: format!("{pool_address:#x}#{}", self.tick_idx),
            tick_idx: BigInt::from(self.tick_idx),
            liquidity_net: BigInt::from_str(&self.liquidity_net).context("parse liquidity_net")?,
            pool_address,
        })
    }
}

#[async_trait]
impl V3PoolDataSource for PoolIndexerClient {
    async fn get_registered_pools(&self) -> Result<RegisteredPools> {
        // Paginate through the full pool set. The block_number returned from
        // the first page is what we pin the snapshot to — subsequent pages
        // may report a higher block, which we tolerate as bounded drift
        // (see block-coherence discussion; driver's event replay takes it
        // from there).
        let mut cursor: Option<String> = None;
        let mut pools: Vec<PoolData> = Vec::new();
        let mut fetched_block_number: u64 = 0;
        loop {
            let mut url = self.path("pools")?;
            url.query_pairs_mut()
                .append_pair("limit", &LIST_PAGE_SIZE.to_string());
            if let Some(c) = &cursor {
                url.query_pairs_mut().append_pair("after", c);
            }
            let page: PoolsResponse = self
                .http
                .get(url)
                .send()
                .await
                .context("GET /pools")?
                .error_for_status()
                .context("pools HTTP status")?
                .json()
                .await
                .context("pools body")?;

            if fetched_block_number == 0 {
                fetched_block_number = page.block_number;
            }
            // Skip zero-liquidity pools (fully-burned LP, never-minted, etc.)
            let filtered = page
                .pools
                .into_iter()
                .filter(|p| p.liquidity != "0")
                .map(PoolData::try_from)
                .collect::<Result<Vec<_>>>()?;
            pools.extend(filtered);
            match page.next_cursor {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        Ok(RegisteredPools {
            fetched_block_number,
            pools,
        })
    }

    async fn get_pools_with_ticks_by_ids(
        &self,
        ids: &[Address],
        _block_number: u64,
    ) -> Result<Vec<PoolData>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut out: Vec<PoolData> = Vec::with_capacity(ids.len());
        for batch in ids.chunks(POOL_IDS_PER_REQUEST) {
            let (pools, ticks_by_pool) = futures::try_join!(
                fetch_pools_by_ids(self, batch),
                fetch_ticks_by_pool_ids(self, batch),
            )?;

            for mut pool in pools {
                if let Some(ticks) = ticks_by_pool.get(&pool.id) {
                    pool.ticks = Some(ticks.clone());
                }
                out.push(pool);
            }
        }
        Ok(out)
    }
}

fn ids_param(ids: &[Address]) -> String {
    ids.iter()
        .map(|a| format!("{a:#x}"))
        .collect::<Vec<_>>()
        .join(",")
}

async fn fetch_pools_by_ids(client: &PoolIndexerClient, ids: &[Address]) -> Result<Vec<PoolData>> {
    let mut url = client.path("pools")?;
    url.query_pairs_mut()
        .append_pair("pool_ids", &ids_param(ids));
    let resp: PoolsResponse = client
        .http
        .get(url)
        .send()
        .await
        .context("GET /pools?pool_ids=")?
        .error_for_status()
        .context("pools-by-ids HTTP status")?
        .json()
        .await
        .context("pools-by-ids body")?;
    resp.pools.into_iter().map(PoolData::try_from).collect()
}

async fn fetch_ticks_by_pool_ids(
    client: &PoolIndexerClient,
    ids: &[Address],
) -> Result<HashMap<Address, Vec<TickData>>> {
    let mut url = client.path("pools/ticks")?;
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
    let mut out: HashMap<Address, Vec<TickData>> = HashMap::new();
    for IndexerPoolTicks { pool, ticks } in resp.pools {
        let mapped: Result<Vec<_>> = ticks.into_iter().map(|t| t.into_tick_data(pool)).collect();
        out.insert(pool, mapped?);
    }
    Ok(out)
}
