use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    serde_json::json,
    std::{str::FromStr, sync::Arc},
};

pub async fn get_total_surplus_handler(
    State(state): State<Arc<AppState>>,
    Path(user): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(user) = Address::from_str(&user) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let surplus = state.database_read.total_surplus(&user).await;
    match surplus {
        Ok(surplus) => (
            StatusCode::OK,
            Json(json!({
                "totalSurplus": surplus.to_string()
            })),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, ?user, "failed to compute total surplus");
            crate::api::internal_error_reply()
        }
    }
}
