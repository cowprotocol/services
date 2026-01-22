use {
    crate::api::AppState,
    alloy::primitives::Address,
    axum::{extract::{Path, State}, http::StatusCode, response::{IntoResponse, Json}},
    serde_json::json,
    std::sync::Arc,
};

pub async fn get_total_surplus_handler(
    State(state): State<Arc<AppState>>,
    Path(user): Path<Address>,
) -> impl IntoResponse {
    let surplus = state.database_read.total_surplus(&user).await;
    match surplus {
        Ok(surplus) => (
            StatusCode::OK,
            Json(json!({
                "totalSurplus": surplus.to_string()
            })),
        ).into_response(),
        Err(err) => {
            tracing::error!(?err, ?user, "failed to compute total surplus");
            crate::api::internal_error_reply()
        }
    }
}
