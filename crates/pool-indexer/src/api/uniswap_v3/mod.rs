pub mod pools;
pub mod ticks;

pub use pools::get_pools;
pub use ticks::get_ticks;

use {
    alloy_primitives::Address,
    axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    },
};

pub(super) fn internal_error(err: anyhow::Error) -> Response {
    tracing::error!(?err, "internal error");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

pub(super) fn parse_hex_address(s: &str) -> Result<Address, &'static str> {
    s.parse::<Address>().map_err(|_| "invalid address")
}
