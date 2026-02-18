use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        response::{IntoResponse, Json, Response},
    },
    std::sync::Arc,
};

pub async fn get_token_metadata_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<Address>,
) -> Response {
    let result = state.database_read.token_metadata(&token).await;
    match result {
        Ok(metadata) => Json(metadata).into_response(),
        Err(err) => {
            tracing::error!(?err, ?token, "Failed to fetch token's first trade block");
            crate::api::internal_error_reply()
        }
    }
}
