//! The order status endpoint: auction progress by uid.

pub mod dto;

use {
    crate::infra::{
        api::{State, error, extract},
        db,
    },
    axum::{Json, http::StatusCode},
};

/// Handle `GET /api/v1/orders/{uid}/status`.
pub async fn order_status(
    state: axum::extract::State<State>,
    extract::PathUid(uid): extract::PathUid,
) -> Result<Json<dto::Status>, error::Reply> {
    let internal = |err: anyhow::Error| {
        tracing::error!(?err, "order status lookup failed");
        error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
    };
    // A trade outranks the event log so an executed order stays traded even
    // when a later event said otherwise.
    if db::order_has_trade(state.pool(), uid)
        .await
        .map_err(internal)?
    {
        return Ok(Json(dto::Status::Traded));
    }
    if let Some(label) = db::latest_order_event(state.pool(), uid)
        .await
        .map_err(internal)?
    {
        return Ok(Json(dto::Status::from_label(&label)));
    }
    // Orders are created on-chain, an indexed order can predate its first
    // auction event.
    if db::order_exists(state.pool(), uid)
        .await
        .map_err(internal)?
    {
        return Ok(Json(dto::Status::Scheduled));
    }
    Err(error::reply(
        StatusCode::NOT_FOUND,
        "NotFound",
        "Order was not found",
    ))
}
