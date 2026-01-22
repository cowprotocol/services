use {
    crate::api::AppState,
    app_data::{AppDataDocument, AppDataHash},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json},
    },
    std::sync::Arc,
};

pub async fn get_app_data_handler(
    State(state): State<Arc<AppState>>,
    Path(contract_app_data): Path<AppDataHash>,
) -> impl IntoResponse {
    let result = state
        .database_read
        .get_full_app_data(&contract_app_data)
        .await;
    match result {
        Ok(Some(response)) => (
            StatusCode::OK,
            Json(AppDataDocument {
                full_app_data: response,
            }),
        )
            .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "full app data not found").into_response(),
        Err(err) => {
            tracing::error!(?err, "get_app_data_by_hash");
            crate::api::internal_error_reply()
        }
    }
}
