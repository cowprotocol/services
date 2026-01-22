use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json},
    },
    std::sync::Arc,
};

pub async fn get_token_metadata_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<Address>,
) -> impl IntoResponse {
    let result = state.database_read.token_metadata(&token).await;
    match result {
        Ok(metadata) => (StatusCode::OK, Json(metadata)).into_response(),
        Err(err) => {
            tracing::error!(?err, ?token, "Failed to fetch token's first trade block");
            crate::api::internal_error_reply()
        }
    }
}
