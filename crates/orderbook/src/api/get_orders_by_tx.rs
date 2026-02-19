use {
    crate::api::AppState,
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        response::{IntoResponse, Json, Response},
    },
    std::sync::Arc,
};

pub async fn get_orders_by_tx_handler(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<B256>,
) -> Response {
    let result = state.orderbook.get_orders_for_tx(&hash).await;
    match result {
        Ok(response) => Json(response).into_response(),
        Err(err) => {
            tracing::error!(?err, "get_orders_by_tx");
            crate::api::internal_error_reply()
        }
    }
}
