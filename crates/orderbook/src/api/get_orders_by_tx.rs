use {
    crate::api::AppState,
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json},
    },
    std::sync::Arc,
};

pub async fn get_orders_by_tx_handler(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<B256>,
) -> impl IntoResponse {
    let result = state.orderbook.get_orders_for_tx(&hash).await;
    match result {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(err) => {
            tracing::error!(?err, "get_orders_by_tx");
            crate::api::internal_error_reply()
        }
    }
}
