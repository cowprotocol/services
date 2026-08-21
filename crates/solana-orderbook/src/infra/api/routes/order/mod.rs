//! The order endpoint: one order with its fill state by uid.

pub mod dto;

use {
    crate::infra::{
        api::{State, error},
        db,
    },
    axum::{Json, extract::Path, http::StatusCode},
    std::time::{SystemTime, UNIX_EPOCH},
};

/// Handle `GET /api/v1/orders/{uid}`. The uid is the order's 32-byte intent
/// hash as `0x`-prefixed hex.
pub async fn order(
    state: axum::extract::State<State>,
    Path(uid): Path<String>,
) -> Result<Json<dto::Order>, error::Reply> {
    let uid = parse_uid(&uid).ok_or_else(|| {
        error::reply(
            StatusCode::BAD_REQUEST,
            "InvalidOrderUid",
            "orderUid must be 32 bytes of hex",
        )
    })?;
    let row = db::order_by_uid(state.pool(), uid)
        .await
        .map_err(|err| {
            tracing::error!(?err, "order lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?
        .ok_or_else(|| error::reply(StatusCode::NOT_FOUND, "NotFound", "Order was not found"))?;
    Ok(Json(dto::Order::new(row, now_unix())))
}

fn parse_uid(uid: &str) -> Option<[u8; 32]> {
    const_hex::decode_to_array(uid).ok()
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after the unix epoch")
        .as_secs()
        .try_into()
        .expect("unix seconds fit i64")
}
