use {
    crate::api::AppState,
    axum::{
        extract::{Path, State},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Json, Response},
    },
    model::order::OrderUid,
    std::{collections::HashMap, sync::Arc},
};

pub async fn debug_order_handler(
    State(state): State<Arc<AppState>>,
    Path(uid): Path<OrderUid>,
    headers: HeaderMap,
) -> Response {
    if state.debug_route_auth_tokens.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "debug endpoint is not enabled"),
        )
            .into_response();
    }

    let token_name = match authenticate(&headers, &state.debug_route_auth_tokens) {
        Some(name) => name,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                super::error("Unauthorized", "invalid or missing x-auth-token"),
            )
                .into_response();
        }
    };

    tracing::info!(%uid, token_name, "debug report requested");

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

fn authenticate<'a>(headers: &HeaderMap, tokens: &'a HashMap<String, String>) -> Option<&'a str> {
    let header_value = headers.get("x-auth-token")?.to_str().ok()?;
    tokens.get(header_value).map(String::as_str)
}
