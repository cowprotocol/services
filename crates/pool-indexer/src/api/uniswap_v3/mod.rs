pub mod pools;
pub mod ticks;

use {
    alloy_primitives::Address,
    axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    },
};
pub use {pools::get_pools, ticks::get_ticks};

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

pub(super) fn parse_hex_address(s: &str) -> Result<Address, &'static str> {
    s.parse::<Address>().map_err(|_| "invalid address")
}
