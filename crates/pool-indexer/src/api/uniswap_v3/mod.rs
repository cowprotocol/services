pub mod bulk_ticks;
pub mod pool_ticks;
pub mod pools_by_ids;
pub mod pools_list;

use {
    crate::db::uniswap_v3 as db,
    alloy_primitives::Address,
    axum::{
        Json,
        response::{IntoResponse, Response},
    },
    bigdecimal::{BigDecimal, num_bigint::ToBigInt},
    serde::{Deserialize, Deserializer, Serialize},
};
pub use {
    bulk_ticks::get_ticks_bulk,
    pool_ticks::get_ticks,
    pools_by_ids::get_pools_by_ids,
    pools_list::get_pools,
};

/// Max pool addresses per bulk lookup. Keeps URLs under proxy limits and
/// caps the DB query size.
pub(super) const MAX_POOL_IDS_PER_REQUEST: usize = 500;

/// Deserializes `?pool_ids=0x…,0x…` into typed addresses. Parsing and the
/// cap happen at the extractor so handlers see a `Vec<Address>`.
pub(crate) struct PoolIds(pub Vec<Address>);

impl<'de> Deserialize<'de> for PoolIds {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(de)?;
        let out: Vec<Address> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|entry| {
                entry
                    .parse::<Address>()
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

/// One tick with its net liquidity delta. Used by both tick endpoints
/// and embedded in [`PoolResponse`] when ticks are requested inline.
#[derive(Serialize)]
pub struct TickEntry {
    pub tick_idx: i32,
    #[serde(serialize_with = "serialize_integer")]
    pub liquidity_net: BigDecimal,
}

impl From<db::TickRow> for TickEntry {
    fn from(tick: db::TickRow) -> Self {
        Self {
            tick_idx: tick.tick_idx,
            liquidity_net: tick.liquidity_net,
        }
    }
}

/// ERC-20 token metadata embedded in pool responses.
#[derive(Serialize)]
pub struct TokenInfo {
    pub id: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// A single Uniswap v3 pool.
#[derive(Serialize)]
pub struct PoolResponse {
    pub id: Address,
    pub token0: TokenInfo,
    pub token1: TokenInfo,
    /// Fee tier in hundredths of a basis point (e.g. 3000 = 0.3%).
    #[serde(serialize_with = "serialize_display")]
    pub fee_tier: u32,
    #[serde(serialize_with = "serialize_integer")]
    pub liquidity: BigDecimal,
    #[serde(serialize_with = "serialize_integer")]
    pub sqrt_price: BigDecimal,
    pub tick: i32,
    /// Set only when ticks are requested inline.
    pub ticks: Option<Vec<TickEntry>>,
}

#[derive(Serialize)]
pub struct PoolsResponse {
    /// Latest fully-indexed block.
    pub block_number: u64,
    pub pools: Vec<PoolResponse>,
    /// Pass as `after=` to fetch the next page; `null` on the last page.
    pub next_cursor: Option<String>,
}

impl From<&db::PoolRow> for PoolResponse {
    fn from(r: &db::PoolRow) -> Self {
        Self {
            id: r.address,
            token0: TokenInfo {
                id: r.token0,
                decimals: r.token0_decimals,
                symbol: non_empty(r.token0_symbol.as_deref()),
            },
            token1: TokenInfo {
                id: r.token1,
                decimals: r.token1_decimals,
                symbol: non_empty(r.token1_symbol.as_deref()),
            },
            fee_tier: r.fee,
            liquidity: r.liquidity.clone(),
            sqrt_price: r.sqrt_price_x96.clone(),
            tick: r.tick,
            ticks: None,
        }
    }
}

/// The symbol backfill writes `""` as a "tried and failed" sentinel.
/// Surface that as missing instead of as an empty string.
pub(super) fn non_empty(s: Option<&str>) -> Option<String> {
    s.filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Shared `PoolsResponse` builder for the listing endpoints.
pub(super) fn pools_response(
    block_number: u64,
    rows: &[db::PoolRow],
    next_cursor: Option<String>,
) -> Response {
    Json(PoolsResponse {
        block_number,
        pools: rows.iter().map(PoolResponse::from).collect(),
        next_cursor,
    })
    .into_response()
}

pub(super) fn serialize_display<T: std::fmt::Display, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// Emits a `BigDecimal` as plain digits. `Display` falls back to `Ne+M`
/// notation for some magnitudes, which `alloy::U256::from_str` rejects.
/// All columns we serialize this way (`sqrt_price_x96`, `liquidity`,
/// `liquidity_net`) hold integers, so the `to_bigint` round-trip is lossless.
pub(super) fn serialize_integer<S: serde::Serializer>(
    value: &BigDecimal,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    // `to_bigint` truncates fractional values; round-trip-check so we don't
    // silently drop precision.
    match value.to_bigint() {
        Some(bi) if BigDecimal::from(bi.clone()) == *value => {
            serializer.serialize_str(&bi.to_string())
        }
        _ => Err(serde::ser::Error::custom(format!(
            "expected integer, got {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        bigdecimal::{BigDecimal, num_bigint::BigInt},
        serde::Serialize,
        std::str::FromStr,
    };

    /// Postgres NUMERIC compresses trailing zeros into a negative
    /// `BigDecimal` scale, which `Display` renders as `Ne+M`. Verify the
    /// serializer emits plain digits so `U256::from_str` can parse the
    /// response on the driver side.
    #[test]
    fn serialize_integer_handles_negative_scale_bigdecimal() {
        let mantissa = BigInt::from_str("79228162514264337593543950336").unwrap();
        let v = BigDecimal::new(mantissa, -30);

        assert_eq!(v.to_string(), "79228162514264337593543950336e+30");

        #[derive(Serialize)]
        struct Wrapper {
            #[serde(serialize_with = "serialize_integer")]
            v: BigDecimal,
        }
        let json = serde_json::to_string(&Wrapper { v }).unwrap();
        assert_eq!(
            json,
            "{\"v\":\"79228162514264337593543950336000000000000000000000000000000\"}"
        );
    }
}
