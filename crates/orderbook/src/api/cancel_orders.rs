use {
    crate::api::AppState,
    axum::{Json, extract::State, http::StatusCode, response::IntoResponse},
    model::order::SignedOrderCancellations,
    std::sync::Arc,
};

pub async fn cancel_orders_handler(
    State(state): State<Arc<AppState>>,
    Json(cancellations): Json<SignedOrderCancellations>,
) -> impl IntoResponse {
    match state.orderbook.cancel_orders(cancellations).await {
        Ok(_) => (StatusCode::OK, Json("Cancelled")).into_response(),
        Err(err) => err.into_response(),
    }
}
