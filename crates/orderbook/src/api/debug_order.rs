use {
    crate::api::AppState,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    std::sync::Arc,
};

pub async fn debug_order_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
) -> Response {
    match state.database_read.fetch_debug_report(&uid).await {
        Ok(Some(report)) => (StatusCode::OK, Json(report)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "order not found"),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, "failed to fetch debug report");
            crate::api::internal_error_reply()
        }
    }
}
