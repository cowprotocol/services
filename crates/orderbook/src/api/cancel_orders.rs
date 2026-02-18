use {
    crate::{api::AppState, orderbook::OrderCancellationError},
    anyhow::anyhow,
    axum::{
        Json,
        body,
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    model::order::{ORDER_UID_LIMIT, SignedOrderCancellations},
    std::sync::Arc,
};

pub async fn cancel_orders_handler(
    State(state): State<Arc<AppState>>,
    body: body::Bytes,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(cancellations) = serde_json::from_slice::<SignedOrderCancellations>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    // Explicitly limit the number of orders cancelled in a batch as the request
    // size limit *does not* provide a proper bound for this
    if cancellations.data.order_uids.len() > ORDER_UID_LIMIT {
        return Err::<&'static str, _>(OrderCancellationError::Other(anyhow!(
            "too many orders ({} > 1024)",
            cancellations.data.order_uids.len()
        )))
        .into_response();
    }

    state
        .orderbook
        .cancel_orders(cancellations)
        .await
        .map(|_| Json("Cancelled"))
        .into_response()
}
