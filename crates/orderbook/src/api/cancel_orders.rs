use {
    crate::api::{AppState, convert_json_response},
    axum::{Json, extract::State, response::IntoResponse},
    model::order::SignedOrderCancellations,
    std::sync::Arc,
};

pub async fn cancel_orders_handler(
    State(state): State<Arc<AppState>>,
    Json(cancellations): Json<SignedOrderCancellations>,
) -> impl IntoResponse {
    convert_json_response(
        state
            .orderbook
            .cancel_orders(cancellations)
            .await
            .map(|_| "Cancelled"),
    )
}
