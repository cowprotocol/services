pub mod pools;
pub mod ticks;

use {
    alloy_primitives::Address,
    bigdecimal::{BigDecimal, num_bigint::ToBigInt},
    serde::{Deserialize, Deserializer},
};
pub use {
    pools::get_pools,
    ticks::{get_ticks, get_ticks_bulk},
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
        let raw = <&str>::deserialize(de)?;
        let mut out = Vec::new();
        for entry in raw.split(',').map(str::trim).filter(|s| {
            if s.is_empty() {
                tracing::warn!("pool_ids query contained an empty entry");
                false
            } else {
                true
            }
        }) {
            out.push(
                entry
                    .parse::<Address>()
                    .map_err(|_| serde::de::Error::custom("invalid pool id"))?,
            );
        }
        if out.len() > MAX_POOL_IDS_PER_REQUEST {
            return Err(serde::de::Error::custom(format!(
                "too many pool ids; max {MAX_POOL_IDS_PER_REQUEST}"
            )));
        }
        Ok(PoolIds(out))
    }
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
