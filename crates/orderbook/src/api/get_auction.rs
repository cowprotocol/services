use {
    crate::api::AppState,
    axum::{
        extract::State,
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    std::sync::Arc,
};

pub async fn get_auction_handler(State(state): State<Arc<AppState>>) -> Response {
    let result = state.orderbook.get_auction().await;
    match result {
        Ok(Some(auction)) => Json(auction).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "There is no active auction"),
        )
            .into_response(),
        Err(err) => {
            tracing::error!(?err, "/api/v1/get_auction");
            crate::api::internal_error_reply()
        }
    }
}
