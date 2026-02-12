use {
    crate::api::AppState,
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    std::{str::FromStr, sync::Arc},
};

pub async fn get_orders_by_tx_handler(
    State(state): State<Arc<AppState>>,
    Path(hash): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(hash) = B256::from_str(&hash) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let result = state.orderbook.get_orders_for_tx(&hash).await;
    match result {
        Ok(response) => Json(response).into_response(),
        Err(err) => {
            tracing::error!(?err, "get_orders_by_tx");
            crate::api::internal_error_reply()
        }
    }
}
