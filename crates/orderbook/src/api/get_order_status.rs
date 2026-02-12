use {
    crate::{api::AppState, orderbook::OrderStatusError},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    std::{str::FromStr, sync::Arc},
};

pub async fn get_status_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(uid) = OrderUid::from_str(&uid) else {
        return StatusCode::NOT_FOUND.into_response();
    };

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
