use {
    crate::api::AppState,
    axum::{
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    std::sync::Arc,
};

pub async fn get_ready_handler(State(state): State<Arc<AppState>>) -> Response {
    match state.database_write.last_used_auction_id().await {
        Ok(_maybe_id) => StatusCode::OK.into_response(),
        Err(err) => {
            tracing::error!(?err, "/api/v1/ready");
            crate::api::internal_error_reply()
        }
    }
}
