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
    tracing::info,
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

/// Executes a GraphQL query against `url` and deserialises the `data` field.
/// Returns an error if the response contains a top-level `errors` array.
async fn gql<T: for<'de> Deserialize<'de>>(
    client: &Client,
    url: &str,
    query: &str,
    vars: Value,
) -> Result<T> {
    let resp = client
        .post(url)
        .json(&json!({ "query": query, "variables": vars }))
        .send()
        .await
        .context("subgraph HTTP request")?;
    let gql_resp: GqlResponse = resp.json().await.context("decode subgraph response")?;
    if let Some(errors) = gql_resp.errors {
        bail!("subgraph errors: {errors}");
    }
    let data = gql_resp.data.context("missing data field")?;
    serde_json::from_value(data).context("decode subgraph data")
}

async fn fetch_current_block(client: &Client, url: &str) -> Result<u64> {
    let page: MetaPage = gql(client, url, "{ _meta { block { number } } }", json!({})).await?;
    Ok(page.meta.block.number)
}

/// Fetches one page of pools at `block`, ordered by id and starting after
/// `cursor` (empty string to start from the beginning).
async fn fetch_pools_page(
    client: &Client,
    url: &str,
    block: u64,
    cursor: &str,
) -> Result<Vec<SubgraphPool>> {
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
    let page: PoolsPage = gql(
        client,
        url,
        query,
        json!({ "block": block, "cursor": cursor }),
    )
    .await?;
    Ok(page.pools)
}

/// Fetches all ticks for `pool_id` at `block` using keyset pagination.
/// Returns each tick as a [`TickDeltaData`] where `delta` is the subgraph's
/// `liquidityNet` (treated as an absolute value, not a running delta).
async fn fetch_ticks_for_pool(
    client: Client,
    url: String,
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

    let pool_addr: Address = pool_id.parse().context("parse pool address")?;
    let mut ticks = Vec::new();
    let mut cursor: i64 = TICK_IDX_CURSOR_START;

    loop {
        let page: TicksPage = gql(
            &client,
            &url,
            query,
            json!({ "pool": pool_id, "cursor": cursor, "block": block }),
        )
        .await?;

        let n = page.ticks.len();
        for t in &page.ticks {
            ticks.push(TickDeltaData {
                pool_address: pool_addr,
                tick_idx: t.tick_idx.parse().context("parse tickIdx")?,
                delta: t.liquidity_net.parse().context("parse liquidityNet")?,
            });
        }

        if n < PAGE_SIZE {
            break;
        }
        cursor = ticks.last().unwrap().tick_idx as i64;
    }

    Ok(ticks)
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
    let client = Client::builder()
        .timeout(SUBGRAPH_REQUEST_TIMEOUT)
        .build()
        .context("build HTTP client")?;

    let block = match block {
        Some(b) => b,
        None => fetch_current_block(&client, subgraph_url)
            .await
            .context("fetch current subgraph block")?,
    };
    info!(block, "seeding pool-indexer from subgraph");

    let mut pool_ids: Vec<String> = Vec::new();
    let mut cursor = String::new();

    loop {
        let page = fetch_pools_page(&client, subgraph_url, block, &cursor).await?;
        let n = page.len();

        let mut new_pools = Vec::with_capacity(n);
        let mut pool_states = Vec::with_capacity(n);

        for p in &page {
            let address: Address = p.id.parse().context("parse pool id")?;
            new_pools.push(NewPoolData {
                address,
                token0: p.token0.id.parse().context("parse token0")?,
                token1: p.token1.id.parse().context("parse token1")?,
                fee: p.fee_tier.parse().context("parse feeTier")?,
                token0_decimals: p.token0.decimals.parse::<u8>().ok(),
                token1_decimals: p.token1.decimals.parse::<u8>().ok(),
                token0_symbol: p.token0.symbol.clone(),
                token1_symbol: p.token1.symbol.clone(),
                created_block: p
                    .created_at_block_number
                    .parse()
                    .context("parse createdAtBlockNumber")?,
            });

            if let Some(tick_str) = &p.tick
                && p.sqrt_price != "0"
            {
                pool_states.push(PoolStateData {
                    pool_address: address,
                    block_number: block,
                    sqrt_price_x96: p.sqrt_price.parse::<U160>().context("parse sqrtPrice")?,
                    liquidity: p.liquidity.parse().context("parse liquidity")?,
                    tick: tick_str.parse().context("parse tick")?,
                });
            }

            pool_ids.push(p.id.clone());
        }

        let mut tx = db.begin().await.context("begin pool tx")?;
        db::batch_insert_pools(&mut tx, chain_id, &new_pools).await?;
        db::batch_upsert_pool_states(&mut tx, chain_id, &pool_states).await?;
        tx.commit().await.context("commit pool tx")?;

        info!(total = pool_ids.len(), "pools seeded");

        if n < PAGE_SIZE {
            break;
        }
        cursor = page.last().unwrap().id.clone();
    }

    info!(
        total = pool_ids.len(),
        "all pools seeded — starting tick seeding"
    );

    // Clear all existing tick data so seeded values are authoritative.
    // This prevents stale rows (e.g. ticks burned to 0 before the seed block)
    // from persisting if the seeder is re-run on a non-empty database.
    db::delete_ticks_for_chain(db, chain_id).await?;

    let mut total_ticks = 0usize;
    let url = subgraph_url.to_owned();

    for chunk in pool_ids.chunks(TICK_CONCURRENCY) {
        let tick_batches: Vec<Vec<TickDeltaData>> = futures::stream::iter(chunk.iter().cloned())
            .map(|pool_id| fetch_ticks_for_pool(client.clone(), url.clone(), pool_id, block))
            .buffer_unordered(TICK_CONCURRENCY)
            .try_collect()
            .await?;

        let ticks: Vec<TickDeltaData> = tick_batches.into_iter().flatten().collect();
        let n = ticks.len();

        if !ticks.is_empty() {
            db::batch_seed_ticks(db, chain_id, &ticks).await?;
        }

        total_ticks += n;
        info!(total = total_ticks, "ticks seeded");
    }

    info!(
        block,
        pools = pool_ids.len(),
        ticks = total_ticks,
        "seeding complete"
    );
    Ok(block)
}
