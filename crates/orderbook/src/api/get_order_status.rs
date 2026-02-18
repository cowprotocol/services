use {
    crate::{api::AppState, orderbook::OrderStatusError},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    std::sync::Arc,
};

pub async fn get_status_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
) -> Response {
    let status = state.orderbook.get_order_status(&uid).await;
    match status {
        Ok(status) => Json(status).into_response(),
        Err(err @ OrderStatusError::NotFound) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", err.to_string()),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, "get_order_status");
            crate::api::internal_error_reply()
        }
    }
}
