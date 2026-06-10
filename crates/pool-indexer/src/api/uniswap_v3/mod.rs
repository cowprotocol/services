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

/// Upper bound on pool addresses accepted in a single bulk lookup. Keeps the
/// URL under typical proxy limits and bounds DB query size.
pub(super) const MAX_POOL_IDS_PER_REQUEST: usize = 500;

/// Newtype over `Vec<Address>` that deserializes from a comma-separated list
/// of 20-byte hex addresses in URL query strings (`0x…,0x…`). Parsing +
/// capping happen at the extractor boundary so handlers work with typed
/// addresses instead of raw strings.
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

/// A single tick entry with its net liquidity. Shared between the
/// single-pool and bulk tick endpoints (and embedded in [`PoolResponse`]
/// when ticks are requested inline).
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
    /// Checksummed contract address.
    pub id: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// A single Uniswap v3 pool. Used by both pool-listing endpoints.
#[derive(Serialize)]
pub struct PoolResponse {
    /// Checksummed pool contract address.
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
    /// Populated only when tick data is explicitly requested.
    pub ticks: Option<Vec<TickEntry>>,
}

/// Response envelope for pool listing and search endpoints.
#[derive(Serialize)]
pub struct PoolsResponse {
    /// Latest block that has been fully indexed.
    pub block_number: u64,
    pub pools: Vec<PoolResponse>,
    /// Cursor to pass as `after` to fetch the next page; `null` on the last
    /// page.
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

/// Empty strings are a "tried-and-failed" sentinel written by the symbol
/// backfill task; surface them as missing rather than as `""`.
pub(super) fn non_empty(s: Option<&str>) -> Option<String> {
    s.filter(|s| !s.is_empty()).map(str::to_owned)
}

/// Converts a slice of DB rows into the on-the-wire [`PoolsResponse`]
/// envelope, attaching the indexed-block tag and optional pagination
/// cursor. Centralised here so every route emits the same JSON shape.
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

/// Serializes any [`Display`](std::fmt::Display) value as a JSON string.
pub(super) fn serialize_display<T: std::fmt::Display, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

/// Serializes a [`BigDecimal`] holding an integer value as a plain decimal
/// string — never scientific notation. `BigDecimal`'s own `Display` emits
/// `"Ne±M"` for some magnitudes, which breaks downstream parsers expecting
/// `uint` strings (alloy's `U256::from_str`). The stored columns
/// (`sqrt_price_x96`, `liquidity`, `liquidity_net`) are always integers, so
/// converting via `BigInt` is lossless.
pub(super) fn serialize_integer<S: serde::Serializer>(
    value: &BigDecimal,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    // `to_bigint` truncates fractional values (1.5 → 1), so also verify the
    // round-trip matches — otherwise we'd silently drop precision.
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

    /// Postgres' NUMERIC wire encoding compresses trailing zeros into a
    /// negative `BigDecimal` scale (`mantissa × 10^|scale|`). The default
    /// `Display` stringifies this as scientific notation (`1E30`), which
    /// `alloy::U256::from_str` rejects — `serialize_integer` must emit
    /// plain digits instead.
    #[test]
    fn serialize_integer_handles_negative_scale_bigdecimal() {
        // negative-scale compression large enough to push `BigDecimal`'s `Display` into
        // `Ne+M` notation.
        let mantissa = BigInt::from_str("79228162514264337593543950336").unwrap();
        let v = BigDecimal::new(mantissa, -30);

        // Confirm the bug shape: default `Display` produces scientific
        // notation that `U256::from_str` can't parse.
        assert_eq!(v.to_string(), "79228162514264337593543950336e+30");

        // Our serializer normalizes to pure digits that the driver parses.
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
