use {
    crate::{api::AppState, orderbook::OrderCancellationError},
    anyhow::anyhow,
    axum::{Json, extract::State},
    model::order::SignedOrderCancellations,
    std::sync::Arc,
};

pub async fn cancel_orders_handler(
    State(state): State<Arc<AppState>>,
    Json(cancellations): Json<SignedOrderCancellations>,
) -> Result<Json<&'static str>, OrderCancellationError> {
    // Explicitly limit the number of orders cancelled in a batch as the request
    // size limit *does not* provide a proper bound for this
    if cancellations.data.order_uids.len() > 1024 {
        return Err(OrderCancellationError::Other(anyhow!(
            "too many orders ({} > 1024)",
            cancellations.data.order_uids.len()
        )));
    }

    state
        .orderbook
        .cancel_orders(cancellations)
        .await
        .map(|_| Json("Cancelled"))
}
