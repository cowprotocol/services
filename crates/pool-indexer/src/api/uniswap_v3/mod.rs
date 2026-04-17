pub mod pools;
pub mod ticks;

use {
    alloy_primitives::Address,
    axum::{
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
};
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

pub(super) fn internal_error(err: anyhow::Error) -> Response {
    tracing::error!(?err, "internal error");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

pub(super) fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message.into() })),
    )
        .into_response()
}

pub(super) fn parse_hex_address(s: &str) -> Result<Address, &'static str> {
    s.parse::<Address>().map_err(|_| "invalid address")
}

/// Parses a comma-separated list of pool addresses (`0x…,0x…`). Empty entries
/// are skipped. Returns a [`Response`] directly on parse failure or when the
/// list exceeds [`MAX_POOL_IDS_PER_REQUEST`] so handlers can `?`-propagate the
/// error response.
#[allow(clippy::result_large_err)]
pub(super) fn parse_pool_ids(raw: &str) -> Result<Vec<Address>, Response> {
    let mut out = Vec::new();
    for entry in raw.split(',').filter(|s| !s.is_empty()) {
        let addr = parse_hex_address(entry.trim()).map_err(|_| bad_request("invalid pool id"))?;
        out.push(addr);
    }
    if out.len() > MAX_POOL_IDS_PER_REQUEST {
        return Err(bad_request(format!(
            "too many pool ids; max {MAX_POOL_IDS_PER_REQUEST}"
        )));
    }
    Ok(out)
}
