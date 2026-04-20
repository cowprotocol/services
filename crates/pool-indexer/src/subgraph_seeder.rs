//! Bootstraps the pool-indexer database from a Uniswap V3 subgraph.
//!
//! Seeding happens in two phases:
//!
//! 1. **Pools** — all pools and their current state are fetched with keyset
//!    pagination and written to the DB in page-sized transactions.
//! 2. **Ticks** — existing tick rows are cleared, then each pool's ticks are
//!    fetched concurrently (up to [`TICK_CONCURRENCY`] at a time) and written.
//!
//! Both phases query the subgraph at the same fixed block number so the
//! snapshot is consistent. After seeding, the caller should invoke
//! `UniswapV3Indexer::catch_up` to replay any blocks the subgraph has already
//! processed but that aren't yet in the DB.

use {
    crate::{
        db::uniswap_v3 as db,
        indexer::uniswap_v3::{NewPoolData, PoolStateData, TickDeltaData},
    },
    alloy_primitives::{Address, aliases::U160},
    anyhow::{Context, Result, bail},
    futures::{StreamExt, TryStreamExt},
    reqwest::Client,
    serde::Deserialize,
    serde_json::{Value, json},
    sqlx::PgPool,
    std::time::Duration,
    tracing::{info, instrument},
};

/// Number of pools (or ticks) returned per GraphQL page.
const PAGE_SIZE: usize = 1000;
/// Maximum number of pools whose ticks are fetched concurrently.
const TICK_CONCURRENCY: usize = 50;
/// Timeout for individual subgraph HTTP requests.
const SUBGRAPH_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Cursor value below the minimum Uniswap V3 tick index (-887272), ensuring the
/// first GraphQL page includes the lowest possible tick.
const TICK_IDX_CURSOR_START: i64 = -887_273;

#[derive(Deserialize)]
struct GqlResponse {
    data: Option<Value>,
    errors: Option<Value>,
}

#[derive(Deserialize)]
struct PoolsPage {
    pools: Vec<SubgraphPool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubgraphPool {
    id: String,
    token0: SubgraphToken,
    token1: SubgraphToken,
    fee_tier: String,
    created_at_block_number: String,
    sqrt_price: String,
    liquidity: String,
    tick: Option<String>,
}

#[derive(Deserialize)]
struct SubgraphToken {
    id: String,
    decimals: String,
    symbol: Option<String>,
}

#[derive(Deserialize)]
struct TicksPage {
    ticks: Vec<SubgraphTick>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubgraphTick {
    tick_idx: String,
    liquidity_net: String,
}

/// Wrapper around the `{ _meta { block { number } } }` GraphQL response.
#[derive(Deserialize)]
struct MetaPage {
    #[serde(rename = "_meta")]
    meta: MetaInfo,
}

#[derive(Deserialize)]
struct MetaInfo {
    block: MetaBlock,
}

#[derive(Deserialize)]
struct MetaBlock {
    number: u64,
}

#[derive(Clone)]
struct SubgraphClient {
    http: Client,
    url: String,
}

impl SubgraphClient {
    fn new(url: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(SUBGRAPH_REQUEST_TIMEOUT)
            .build()
            .context("build HTTP client")?;

        Ok(Self {
            http,
            url: url.to_owned(),
        })
    }

    /// Executes a GraphQL query and deserialises the `data` field.
    /// Returns an error if the response contains a top-level `errors` array.
    async fn query<T: for<'de> Deserialize<'de>>(&self, query: &str, vars: Value) -> Result<T> {
        let response = self
            .http
            .post(&self.url)
            .json(&json!({ "query": query, "variables": vars }))
            .send()
            .await
            .context("subgraph HTTP request")?;

        let gql_response: GqlResponse =
            response.json().await.context("decode subgraph response")?;
        if let Some(errors) = gql_response.errors {
            bail!("subgraph errors: {errors}");
        }

        let data = gql_response.data.context("missing data field")?;
        serde_json::from_value(data).context("decode subgraph data")
    }

    async fn current_block(&self) -> Result<u64> {
        let page: MetaPage = self
            .query("{ _meta { block { number } } }", json!({}))
            .await?;
        Ok(page.meta.block.number)
    }

    /// Fetches one page of pools at `block`, ordered by id and starting after
    /// `cursor` (empty string to start from the beginning).
    async fn fetch_pools_page(&self, block: u64, cursor: &str) -> Result<Vec<SubgraphPool>> {
        let query = "query($block: Int!, $cursor: String!) {
            pools(first: 1000, orderBy: id, where: {id_gt: $cursor}, block: {number: $block}) {
                id
                token0 { id decimals symbol }
                token1 { id decimals symbol }
                feeTier
                createdAtBlockNumber
                sqrtPrice
                liquidity
                tick
            }
        }";

        let page: PoolsPage = self
            .query(query, json!({ "block": block, "cursor": cursor }))
            .await?;
        Ok(page.pools)
    }

    /// Fetches all ticks for `pool_id` at `block` using keyset pagination.
    /// Returns each tick as a [`TickDeltaData`] where `delta` is the subgraph's
    /// `liquidityNet` (treated as an absolute value, not a running delta).
    async fn fetch_ticks_for_pool(
        &self,
        pool_id: String,
        block: u64,
    ) -> Result<Vec<TickDeltaData>> {
        let query = "query($pool: String!, $cursor: Int!, $block: Int!) {
            ticks(
                first: 1000,
                orderBy: tickIdx,
                where: { pool: $pool, tickIdx_gt: $cursor },
                block: { number: $block }
            ) {
                tickIdx
                liquidityNet
            }
        }";

        let pool_address: Address = pool_id.parse().context("parse pool address")?;
        let mut ticks = Vec::new();
        let mut cursor = TICK_IDX_CURSOR_START;

        loop {
            let page: TicksPage = self
                .query(
                    query,
                    json!({ "pool": pool_id, "cursor": cursor, "block": block }),
                )
                .await?;

            for tick in &page.ticks {
                ticks.push(TickDeltaData {
                    pool_address,
                    tick_idx: tick.tick_idx.parse().context("parse tickIdx")?,
                    delta: tick.liquidity_net.parse().context("parse liquidityNet")?,
                });
            }

            if page.ticks.len() < PAGE_SIZE {
                break;
            }

            cursor = ticks.last().expect("tick page is non-empty").tick_idx as i64;
        }

        Ok(ticks)
    }
}

struct SubgraphSeeder<'a> {
    db: &'a PgPool,
    chain_id: u64,
    subgraph: SubgraphClient,
    snapshot_block: u64,
}

impl<'a> SubgraphSeeder<'a> {
    async fn new(
        db: &'a PgPool,
        chain_id: u64,
        subgraph_url: &str,
        block: Option<u64>,
    ) -> Result<Self> {
        let subgraph = SubgraphClient::new(subgraph_url)?;
        let snapshot_block = match block {
            Some(block) => block,
            None => subgraph
                .current_block()
                .await
                .context("fetch current subgraph block")?,
        };

        Ok(Self {
            db,
            chain_id,
            subgraph,
            snapshot_block,
        })
    }

    #[instrument(skip_all, fields(chain_id = self.chain_id))]
    async fn seed(self) -> Result<u64> {
        info!(
            block = self.snapshot_block,
            "seeding pool-indexer from subgraph"
        );

        let pool_ids = self.seed_pools().await?;
        let total_ticks = self.seed_ticks(&pool_ids).await?;

        info!(
            block = self.snapshot_block,
            pools = pool_ids.len(),
            ticks = total_ticks,
            "seeding complete"
        );
        Ok(self.snapshot_block)
    }

    async fn seed_pools(&self) -> Result<Vec<String>> {
        let mut all_pool_ids = Vec::new();
        let mut cursor = String::new();

        loop {
            let page = self
                .subgraph
                .fetch_pools_page(self.snapshot_block, &cursor)
                .await?;
            let page_len = page.len();

            all_pool_ids.extend(self.persist_pool_page(&page).await?);
            info!(total = all_pool_ids.len(), "pools seeded");

            if page_len < PAGE_SIZE {
                break;
            }

            cursor = page.last().expect("full pages are non-empty").id.clone();
        }

        info!(
            total = all_pool_ids.len(),
            "all pools seeded — starting tick seeding"
        );
        Ok(all_pool_ids)
    }

    async fn persist_pool_page(&self, page: &[SubgraphPool]) -> Result<Vec<String>> {
        let mut pool_ids = Vec::with_capacity(page.len());
        let mut new_pools = Vec::with_capacity(page.len());
        let mut pool_states = Vec::with_capacity(page.len());

        for pool in page {
            let (pool_id, new_pool, pool_state) = parse_seeded_pool(pool, self.snapshot_block)?;
            pool_ids.push(pool_id);
            new_pools.push(new_pool);

            if let Some(pool_state) = pool_state {
                pool_states.push(pool_state);
            }
        }

        let mut tx = self.db.begin().await.context("begin pool tx")?;
        db::batch_insert_pools(&mut tx, self.chain_id, &new_pools).await?;
        db::batch_upsert_pool_states(&mut tx, self.chain_id, &pool_states).await?;
        tx.commit().await.context("commit pool tx")?;

        Ok(pool_ids)
    }

    async fn seed_ticks(&self, pool_ids: &[String]) -> Result<usize> {
        // Clear all existing tick data so seeded values are authoritative.
        // This prevents stale rows (e.g. ticks burned to 0 before the seed block)
        // from persisting if the seeder is re-run on a non-empty database.
        db::delete_ticks_for_chain(self.db, self.chain_id).await?;

        let mut total_ticks = 0usize;
        for pool_batch in pool_ids.chunks(TICK_CONCURRENCY) {
            let ticks = self.fetch_tick_batch(pool_batch).await?;

            if !ticks.is_empty() {
                db::batch_seed_ticks(self.db, self.chain_id, &ticks).await?;
            }

            total_ticks += ticks.len();
            info!(total = total_ticks, "ticks seeded");
        }

        Ok(total_ticks)
    }

    async fn fetch_tick_batch(&self, pool_batch: &[String]) -> Result<Vec<TickDeltaData>> {
        let subgraph = self.subgraph.clone();
        let snapshot_block = self.snapshot_block;

        let tick_batches: Vec<Vec<TickDeltaData>> =
            futures::stream::iter(pool_batch.iter().cloned())
                .map(move |pool_id| {
                    let subgraph = subgraph.clone();
                    async move { subgraph.fetch_ticks_for_pool(pool_id, snapshot_block).await }
                })
                .buffer_unordered(TICK_CONCURRENCY)
                .try_collect()
                .await?;

        Ok(tick_batches.into_iter().flatten().collect())
    }
}

fn parse_seeded_pool(
    pool: &SubgraphPool,
    snapshot_block: u64,
) -> Result<(String, NewPoolData, Option<PoolStateData>)> {
    let address: Address = pool.id.parse().context("parse pool id")?;
    let new_pool = NewPoolData {
        address,
        token0: pool.token0.id.parse().context("parse token0")?,
        token1: pool.token1.id.parse().context("parse token1")?,
        fee: pool.fee_tier.parse().context("parse feeTier")?,
        token0_decimals: pool.token0.decimals.parse::<u8>().ok(),
        token1_decimals: pool.token1.decimals.parse::<u8>().ok(),
        token0_symbol: pool.token0.symbol.clone(),
        token1_symbol: pool.token1.symbol.clone(),
        created_block: pool
            .created_at_block_number
            .parse()
            .context("parse createdAtBlockNumber")?,
    };

    Ok((
        pool.id.clone(),
        new_pool,
        parse_seeded_pool_state(pool, address, snapshot_block)?,
    ))
}

fn parse_seeded_pool_state(
    pool: &SubgraphPool,
    address: Address,
    snapshot_block: u64,
) -> Result<Option<PoolStateData>> {
    let Some(tick) = pool.tick.as_deref() else {
        return Ok(None);
    };
    if pool.sqrt_price == "0" {
        return Ok(None);
    }

    Ok(Some(PoolStateData {
        pool_address: address,
        block_number: snapshot_block,
        sqrt_price_x96: pool.sqrt_price.parse::<U160>().context("parse sqrtPrice")?,
        liquidity: pool.liquidity.parse().context("parse liquidity")?,
        tick: tick.parse().context("parse tick")?,
    }))
}

/// Seeds pools and ticks from the subgraph and returns the block number that
/// was seeded. The caller is responsible for catching up to the current
/// finalized block via `catch_up`.
pub async fn seed(
    db: &PgPool,
    chain_id: u64,
    subgraph_url: &str,
    block: Option<u64>,
) -> Result<u64> {
    SubgraphSeeder::new(db, chain_id, subgraph_url, block)
        .await?
        .seed()
        .await
}
