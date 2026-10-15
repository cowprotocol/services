//! The order endpoint: one order with its fill state by uid.

pub mod dto;

use {
    crate::infra::{
        api::{State, error, extract},
        db,
    },
    axum::{Json, http::StatusCode},
    std::time::{SystemTime, UNIX_EPOCH},
};

/// Handle `GET /api/v1/orders/{uid}`.
pub async fn order(
    state: axum::extract::State<State>,
    extract::PathUid(uid): extract::PathUid,
) -> Result<Json<dto::Order>, error::Reply> {
    let row = db::find_order_by_uid(state.pool(), uid)
        .await
        .map_err(|err| {
            tracing::error!(?err, "order lookup failed");
            error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
        })?
        .ok_or_else(|| error::reply(StatusCode::NOT_FOUND, "NotFound", "Order was not found"))?;
    let block_height = block_height_for(&state, &row).await;
    Ok(Json(dto::Order::new(row, now_unix(), block_height)))
}

/// The chain height to check `row`'s creation deadline against. Only a pending
/// creation has one, so other orders skip the RPC call.
pub(super) async fn block_height_for(state: &State, row: &db::OrderRow) -> Option<i64> {
    if row.last_valid_block_height.is_some() {
        state.block_height().await
    } else {
        None
    }
}

pub(super) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after the unix epoch")
        .as_secs()
        .try_into()
        .expect("unix seconds fit i64")
}
