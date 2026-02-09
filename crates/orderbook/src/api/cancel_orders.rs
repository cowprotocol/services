use {
    crate::{api::AppState, orderbook::OrderCancellationError},
    axum::{Json, extract::State},
    model::order::SignedOrderCancellations,
    std::sync::Arc,
};

pub async fn cancel_orders_handler(
    State(state): State<Arc<AppState>>,
    Json(cancellations): Json<SignedOrderCancellations>,
) -> Result<Json<&'static str>, OrderCancellationError> {
    state
        .orderbook
        .cancel_orders(cancellations)
        .await
        .map(|_| Json("Cancelled"))
}
