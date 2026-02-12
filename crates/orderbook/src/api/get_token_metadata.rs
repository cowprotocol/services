use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    std::{str::FromStr, sync::Arc},
};

pub async fn get_token_metadata_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(token) = Address::from_str(&token) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let result = state.database_read.token_metadata(&token).await;
    match result {
        Ok(metadata) => Json(metadata).into_response(),
        Err(err) => {
            tracing::error!(?err, ?token, "Failed to fetch token's first trade block");
            crate::api::internal_error_reply()
        }
    }
}
