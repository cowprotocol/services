pub mod pools_by_ids;
pub mod pools_list;

use {
    crate::db::balancer_v2 as db,
    alloy_primitives::{Address, B256},
    axum::{
        Json,
        response::{IntoResponse, Response},
    },
    serde::{Deserialize, Deserializer, Serialize},
};
pub use {pools_by_ids::get_pools_by_ids, pools_list::get_pools};

/// Max pool ids per bulk lookup. Keeps URLs under proxy limits and caps the DB
/// query size.
pub(super) const MAX_POOL_IDS_PER_REQUEST: usize = 500;

/// Deserializes `?pool_ids=0x…,0x…` into 32-byte pool ids. Parsing and the cap
/// happen in the extractor so handlers see a `Vec<B256>`.
pub(crate) struct PoolIds(pub Vec<B256>);

impl<'de> Deserialize<'de> for PoolIds {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(de)?;
        let out: Vec<B256> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|entry| {
                entry
                    .parse::<B256>()
                    .map_err(|_| serde::de::Error::custom("invalid pool id"))
            })
            .collect::<Result<_, D::Error>>()?;
        if out.len() > MAX_POOL_IDS_PER_REQUEST {
            return Err(serde::de::Error::custom(format!(
                "too many pool ids; max {MAX_POOL_IDS_PER_REQUEST}"
            )));
        }
        Ok(PoolIds(out))
    }
}

/// One token of a pool, in `getPoolTokens` order.
#[derive(Serialize)]
pub struct TokenInfo {
    pub address: Address,
    pub decimals: u8,
    /// Normalized weight as a decimal fraction (e.g. "0.5"); present only for
    /// weighted pools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
}

/// A single Balancer V2 pool. Field names mirror the driver's discovery type.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolResponse {
    pub pool_type: String,
    pub id: B256,
    pub address: Address,
    pub factory: Address,
    /// Discovery-time flag; the driver re-checks it on-chain for LBP pools.
    /// Always `true` here, since the indexer stores static metadata only.
    pub swap_enabled: bool,
    pub tokens: Vec<TokenInfo>,
}

#[derive(Serialize)]
pub struct PoolsResponse {
    /// Latest block every configured balancer factory is indexed through.
    pub block_number: u64,
    pub pools: Vec<PoolResponse>,
    /// Pass as `after=` to fetch the next page; `null` on the last page.
    pub next_cursor: Option<String>,
}

impl From<db::BalancerTokenRow> for TokenInfo {
    fn from(t: db::BalancerTokenRow) -> Self {
        Self {
            address: t.address,
            decimals: t.decimals,
            // `normalized()` strips the trailing zeros Postgres NUMERIC pads on
            // when decoded through its base-10000 digit groups. Without it a
            // weight can render with >18 fractional digits, which the driver's
            // fixed-point parser rejects.
            weight: t.weight.map(|w| w.normalized().to_string()),
        }
    }
}

impl From<db::BalancerPoolRow> for PoolResponse {
    fn from(row: db::BalancerPoolRow) -> Self {
        Self {
            pool_type: row.pool_type,
            id: row.pool_id,
            address: row.address,
            factory: row.factory,
            swap_enabled: true,
            tokens: row.tokens.into_iter().map(TokenInfo::from).collect(),
        }
    }
}

/// Shared `PoolsResponse` builder for the balancer listing endpoints.
pub(super) fn pools_response(
    block_number: u64,
    pools: Vec<db::BalancerPoolRow>,
    next_cursor: Option<String>,
) -> Response {
    Json(PoolsResponse {
        block_number,
        pools: pools.into_iter().map(PoolResponse::from).collect(),
        next_cursor,
    })
    .into_response()
}
