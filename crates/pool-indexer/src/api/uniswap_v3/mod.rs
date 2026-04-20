pub mod pools;
pub mod ticks;

use {crate::api::ApiError, alloy_primitives::Address};
pub use {
    pools::get_pools,
    ticks::{get_ticks, get_ticks_bulk},
};

/// Upper bound on pool addresses accepted in a single bulk lookup. Keeps the
/// URL under typical proxy limits and bounds DB query size.
pub(super) const MAX_POOL_IDS_PER_REQUEST: usize = 500;

/// Serializes any [`Display`](std::fmt::Display) value as a JSON string.
pub(super) fn serialize_display<T: std::fmt::Display, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&value.to_string())
}

pub(super) fn parse_hex_address(s: &str) -> Result<Address, ApiError> {
    s.parse::<Address>()
        .map_err(|_| ApiError::InvalidPoolAddress)
}

/// Parses a comma-separated list of pool addresses (`0x…,0x…`). Empty entries
/// are skipped.
pub(super) fn parse_pool_ids(raw: &str) -> Result<Vec<Address>, ApiError> {
    let mut out = Vec::new();
    for entry in raw.split(',').filter(|s| !s.is_empty()) {
        out.push(
            entry
                .trim()
                .parse::<Address>()
                .map_err(|_| ApiError::InvalidPoolId)?,
        );
    }
    if out.len() > MAX_POOL_IDS_PER_REQUEST {
        return Err(ApiError::TooManyPoolIds {
            max: MAX_POOL_IDS_PER_REQUEST,
        });
    }
    Ok(out)
}
